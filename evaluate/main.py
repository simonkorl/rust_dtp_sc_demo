import os, platform, json
import sys
import time
import re
from tqdm import tqdm

MAX_DATAGRAM_SIZE = 1350 # bytes
MAX_BLOCK_SIZE = 1000000 # 1Mbytes
order_preffix = " " if "windows" in platform.system().lower() else "sudo "


# prepare tmp directory
tmp_dir_path = "./tmp"
if not os.path.exists(tmp_dir_path):
        os.mkdir(tmp_dir_path)
# move logs to log diectory
logs_path = "./logs"
if not os.path.exists(logs_path):
    os.mkdir(logs_path)
results_path = "./results"
if not os.path.exists(results_path):
    os.mkdir(results_path)


def parse_config(filepath):
    '''
    Each config file should be json object of list of configs, like:

    [
        {
            "block_size": 100, # the number of maximum-size datagrams in each block.
                                    # Max block size is 1000000 bytes where this param is approximate 740: int (0, 740]
            "block_num": 1000, # total number of blocks: int
            "redundancy": 0.1, # the rate of redundancy: [0, inf)
            "cc": ["bbr", "reno", "cubic"] # a list of target CC algorithm. If the list is empty, it means all of three CC algorithms.
            "bw": 10, # float: Mbps
            "rtt": 100, # float: ms
            "loss": 0 # float: [0, 1]
        },
        {
            ...
        }
    ]
    '''
    with open(filepath, 'r') as f:
        return json.load(f)

def generate_block_file(config):

    block_size = min(int(config['block_size']) * MAX_DATAGRAM_SIZE, MAX_BLOCK_SIZE)
    if block_size <= 0:
        block_size = MAX_DATAGRAM_SIZE
    block_num = int(config['block_num'])
    with open(os.path.join(tmp_dir_path, 'aitrans_block.txt'), 'w') as f:
        for i in range(block_num):
            f.write('%f    %d    %d    %d\n' % (0.001 * i, 2147483647, block_size, 0))

def generate_network_file(config):
    '''
    generate consistent network trace

    example: 0, 1, 0.00001, 0.1 # @0 second, 1Mbps, loss rate=0.00001, 100ms latency
    '''
    with open(os.path.join(tmp_dir_path, 'traces.txt'), 'w') as f:
        f.write('%d, %f, %f, %f' % (0, float(config['bw']), float(config['loss']), float(config['rtt']) / 2 / 1000))

def generate_redundancy_code_file(config):
    with open(os.path.join(tmp_dir_path, 'redundant.cpp'), 'w') as f:
        f.write( \
            '''
            #include "solution.hxx"
            float global_redundancy = {0};
            '''.format(max(0.0, float(config['redundancy']))) \
        )

# docker settings
server_ip   = '172.17.0.2'
port        = 5555
block_trace = './tmp/aitrans_block.txt'
container_server_name = 'server'
container_client_name = 'client'
docker_run_path = "/home/aitrans-server/"
trace_path  = './tmp/traces.txt'
redundant_path = './tmp/redundant.cpp'
# always use tc limits
tc_preffix_c = ''
tc_preffix_s = ''
# always compile
compile_preffix = ''

image_name = 'simonkorl0228/test_image:yg.{0}'

def prepare_docker_files(config):
    '''
    Copy traces and some other files that are consistent in a single test described in `config`
    '''
    # init block trace
    os.system(order_preffix + "docker cp " + block_trace + ' ' + container_server_name + ":%strace/block_trace/aitrans_block.txt" % (docker_run_path))
    # init network traces
    os.system(order_preffix + "docker cp " + trace_path + ' ' + container_server_name + ":%strace/traces.txt" % (docker_run_path))
    os.system(order_preffix + "docker cp " + trace_path + ' ' + container_client_name + ":%strace/traces.txt" % (docker_run_path))
    # init redundancy
    generate_redundancy_code_file(config)
    os.system(order_preffix + "docker cp " + redundant_path + ' ' + container_server_name + ":%sdemo/redundant.cpp" % (docker_run_path))

def prepare_shell_code():

    client_run_line = 'LD_LIBRARY_PATH=./lib RUST_LOG=debug ./client {0} {1} --no-verify > client_err.log 2>&1'
    client_run_line = client_run_line.format(server_ip, port)

    client_run = '''
    #!/bin/bash
    cd {0}
    {1} python3 traffic_control.py -load trace/traces.txt > tc.log 2>&1 &
    rm client.log > tmp.log 2>&1
    sleep 0.2
    {2}
    {1} python3 traffic_control.py --reset eth0
    '''.format(docker_run_path, tc_preffix_c , client_run_line)

    server_run_line = 'LD_LIBRARY_PATH=./lib RUST_LOG=debug ./bin/server {0} {1} trace/block_trace/aitrans_block.txt &> ./log/server_err.log &'
    server_run_line = server_run_line.format(server_ip, port)

    server_run = '''
    #!/bin/bash
    cd {0}
    {1} python3 traffic_control.py -aft 3.1 -load trace/traces.txt > tc.log 2>&1 &

    cd {0}demo
    {3} rm libsolution.so ../lib/libsolution.so
    {3} g++ -shared -fPIC solution.cxx redundant.cpp -I. -o libsolution.so > compile.log 2>&1
    cp libsolution.so ../lib

    # check port
    a=`lsof -i:{2} | awk '/server/ {{print$2}}'`
    if [ $a > 0 ]; then
        kill -9 $a
    fi

    cd {0}
    rm log/server_aitrans.log
    {4}
    '''.format(docker_run_path, tc_preffix_s, port, compile_preffix, server_run_line)

    with open(tmp_dir_path + "/server_run.sh", "w", newline='\n')  as f:
        f.write(server_run)

    with open(tmp_dir_path + "/client_run.sh", "w", newline='\n') as f:
        f.write(client_run)

# run shell order
order_list = [
    order_preffix + " docker cp ./traffic_control.py " + container_server_name + ":" + docker_run_path,
    order_preffix + " docker cp ./traffic_control.py " + container_client_name + ":" + docker_run_path,
    order_preffix + " docker cp %s/server_run.sh " %(tmp_dir_path) + container_server_name + ":" + docker_run_path,
    order_preffix + " docker cp %s/client_run.sh " %(tmp_dir_path) + container_client_name + ":" + docker_run_path,
    order_preffix + " docker exec -it " + container_server_name + " nohup /bin/bash %sserver_run.sh" % (docker_run_path)
]

def run_dockers():
    global server_ip, order_list
    run_seq = 0
    retry_times = 0
    run_times = 1
    while run_seq < run_times:
        print("The %d round :" % (run_seq))

        print("--restart docker--")
        os.system("docker restart %s %s" % (container_server_name, container_client_name))
        time.sleep(5)
        # get server ip after restart docker
        if not server_ip:
            out = os.popen("docker inspect %s" % (container_server_name)).read()
            out_dt = json.loads(out)
            server_ip = out_dt[0]["NetworkSettings"]["IPAddress"]

        prepare_shell_code()
        for idx, order in enumerate(order_list):
            print(idx, " ", order)
            os.system(order)

        # ensure server established succussfully
        time.sleep(3)
        print("run client")
        os.system(order_preffix + " docker exec -it " + container_client_name + "  /bin/bash %sclient_run.sh" % (docker_run_path))
        # ensure connection closed
        time.sleep(3)

        stop_server = '''
        #!/bin/bash
        cd {0}
        a=`lsof -i:{1} | awk '/server/ {{print$2}}'`
        if [ $a > 0 ]; then
            kill -9 $a
        fi
        {2} kill `ps -ef | grep python | awk '/traffic_control/ {{print $2}}'`
        {2} python3 traffic_control.py --reset eth0
        '''.format(docker_run_path, port, tc_preffix_s)

        with open(tmp_dir_path + "/stop_server.sh", "w", newline='\n')  as f:
            f.write(stop_server)

        print("stop server")
        os.system(order_preffix + " docker cp %s/stop_server.sh " %(tmp_dir_path) + container_server_name + ":%s" % (docker_run_path))
        os.system(order_preffix + " docker exec -it " + container_server_name + "  /bin/bash %sstop_server.sh" % (docker_run_path))
        # move logs
        os.system(order_preffix + " rm -rf logs/*")
        os.system(order_preffix + " docker cp " + container_client_name + ":%sclient.log %s/." % (docker_run_path, logs_path))
        os.system(order_preffix + " docker cp " + container_client_name + ":%sclient_err.log %s/." % (docker_run_path, logs_path))
        os.system(order_preffix + " docker cp " + container_server_name + ":%slog/ %s/." % (docker_run_path, logs_path))
        os.system(order_preffix + " docker cp " + container_server_name + ":%sdemo/compile.log %s/compile.log" % (docker_run_path, logs_path))
        os.system(order_preffix + " docker cp " + container_client_name + ":%stc.log %s/client_tc.log" % (docker_run_path, logs_path))
        os.system(order_preffix + " docker cp " + container_server_name + ":%stc.log %s/server_tc.log" % (docker_run_path, logs_path))
        # move .so file
        os.system(order_preffix + " docker cp " + container_server_name + ":%slib/libsolution.so %s/." % (docker_run_path, logs_path))

        # rerun main.py if server fail to start
        try:
            f = open("%s/client.log" % (logs_path), 'r')
            if len(f.readlines()) <= 5:
                print("server run fail, begin restart!")
                retry_times += 1
                continue
        except:
            print("Can not find %s/client.log, file open fail!" % (logs_path))
        run_seq += 1

def start_dockers(cc_algo):
    os.system(order_preffix + " docker stop " + container_server_name)
    os.system(order_preffix + " docker stop " + container_client_name)
    os.system(order_preffix + " docker rm " + container_server_name)
    os.system(order_preffix + " docker rm " + container_client_name)
    os.system(order_preffix + " docker run --privileged -dit --cap-add=NET_ADMIN --name %s %s" % (container_server_name, image_name.format(cc_algo)))
    os.system(order_preffix + " docker run --privileged -dit --cap-add=NET_ADMIN --name %s %s" % (container_client_name, image_name.format(cc_algo)))

CLIENT_LOG_PATTERN = re.compile(r'connection closed, recv=(-?\d+) sent=(-?\d+) lost=(-?\d+) rtt=(?:(?:(\d|.+)ms)|(?:(-1))) cwnd=(-?\d+), total_bytes=(-?\d+), complete_bytes=(-?\d+), good_bytes=(-?\d+), total_time=(-?\d+)')
CLIENT_STAT_INDEXES = ["c_recv", "c_sent", "c_lost", "c_rtt(ms)", "c_cwnd", "c_total_bytes", "c_complete_bytes", "c_good_bytes", "c_total_time(us)", "qoe", "retry_times"]
CLIENT_BLOCKS_INDEXES = ["BlockID", "bct", "BlockSize", "Priority", "Deadline"]

def parse_client_log(dir_path):
    '''
    Parse client.log and get two dicts of information.

    `client_blocks_dict` stores information in client.log about block's stream_id, bct, deadline and priority
    `client_stat_dict` stores statistics offered in client.log. Some important information is like goodbytes and total running time(total time)
    '''
    # collect client blocks information
    client_blocks_dict = {}
    for index in CLIENT_BLOCKS_INDEXES:
        client_blocks_dict[index] = []
    # collect client stats
    client_stat_dict = {}
    for index in CLIENT_STAT_INDEXES:
        client_stat_dict[index] = []

    with open(os.path.join(dir_path, "client.log")) as client:
        client_lines = client.readlines()

        for line in client_lines[4:-1]:
            if len(line) > 1:
               client_line_list = line.split()
               if len(client_line_list) != len(CLIENT_BLOCKS_INDEXES):
                   print("A client block log line has error format in : %s. This happens sometime." % dir_path)
                   continue
               for i in range(len(client_line_list)):
                   client_blocks_dict[CLIENT_BLOCKS_INDEXES[i]].append(client_line_list[i])

        # try to parse the last line of client log
        try:
            match = CLIENT_LOG_PATTERN.match(client_lines[-1])
            if match == None:
                raise ValueError("client re match returns None in : %s" % dir_path, client_lines[-1])


            client_stat_dict["c_recv"].append(float(match.group(1)))
            client_stat_dict["c_sent"].append(float(match.group(2)))
            client_stat_dict["c_lost"].append(float(match.group(3)))

            if match.group(4) is None:
               client_stat_dict["c_rtt(ms)"].append(float(-1))
            else:
               client_stat_dict["c_rtt(ms)"].append(float(match.group(4)))

            client_stat_dict["c_cwnd"].append(float(match.group(6)))
            client_stat_dict["c_total_bytes"].append(float(match.group(7)))
            client_stat_dict["c_complete_bytes"].append(float(match.group(8)))
            client_stat_dict["c_good_bytes"].append(float(9))
            client_stat_dict["c_total_time(us)"].append(float(match.group(10)))

            # invalid stat
            client_stat_dict["qoe"].append(-1)
            client_stat_dict["retry_times"].append(-1)

            return client_blocks_dict, client_stat_dict

        except Exception:
            print(dir_path)
            print(client_lines[-1])
            if match is not None:
                print(match.groups())
            raise ValueError("Could not parse client's last line")

def parse_client_result(config, cc_algo, store_path):
    '''
    Parse the content in client.log and write the BCT as results in store_path

    Results looks like the following example:

    ```txt
    {"block_size": 50, "block_num": 10, "redundancy": 0.0, "cc": "bbr", "bw": 100, "rtt": 50, "loss": 0} # the current config
    {"bct(ms)": "146"} # bct result of the first block
    {"bct(ms)": "231"} # ...
    ```
    '''
    client_blocks_dict, _ = parse_client_log(logs_path)
    new_config = config.copy()
    new_config['cc'] = cc_algo
    with open(store_path, 'w') as f:
        f.write(json.dumps(new_config))
        f.write('\n')
        for _, bct in enumerate(client_blocks_dict['bct']):
            obj = {
                'bct(ms)': bct
            }
            f.write(json.dumps(obj))
            f.write('\n')

USAGE = '''
Usage: python main.py [config_path]

config_path: path to your config file, default './config.json'
'''
if __name__ == "__main__":
    argv = sys.argv
    # find config.json file
    config_filepath = "config.json"
    if len(argv) == 2:
        config_filepath = argv[1]
    elif len(argv) == 1:
        pass # keep config_filepath
    else:
        print(USAGE)
        quit()

    # parse configs from 'config.json'
    configs = parse_config(config_filepath)
    if type(configs) != list:
        print('Configs parse error: configs should be json list')
    if len(configs) == 0:
        print("There is no config in the file")

    total = 0
    # calculate the number of tests for the progress bar
    for config in configs:
        total += len(config['cc'])
        if len(config['cc']) == 0:
            total += 3
    # start testing
    with tqdm(total=total) as pbar:
        exp_dir = "t" + str(time.time_ns())
        exp_store_dir = os.path.join(results_path, exp_dir)
        if not os.path.exists(exp_store_dir):
            os.mkdir(exp_store_dir)

        for idx, config in enumerate(configs):
            print(config)
            if len(config['cc']) == 0:
                cc = ['bbr', 'reno', 'cubic']
            else:
                cc = config['cc']
            generate_block_file(config)
            generate_network_file(config)
            for cc_algo in cc:
                start_dockers(cc_algo)
                prepare_docker_files(config)
                run_dockers()
                store_path = os.path.join(exp_store_dir, "%d_%s" % (idx, cc_algo))
                parse_client_result(config, cc_algo, store_path)
                pbar.update(1)

        with open(os.path.join(exp_store_dir, 'config.json'), 'w') as f:
            json.dump(config, f)
