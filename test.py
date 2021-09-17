import sys
import os
import time
import json

result_path = "./results"
if not os.path.exists(result_path):
    os.mkdir(result_path)

test_dir = os.path.join(os.path.join(result_path, str(time.time_ns())))
os.mkdir(test_dir)

config = {}
with open('config.json', 'r') as f:
    config = json.load(f)

i = 0
data = [[],[],[]]
while i < config['num']:
    # run test
    os.system(config['cmd'])
    time.sleep(6.2)
    log_dir = os.path.join(test_dir, str(i))
    if not os.path.exists(log_dir):
        os.mkdir(log_dir)
    os.system("cp -rf ./aitrans-server/*.log {}".format(log_dir))
    os.system("cp -rf ./aitrans-server/log {}".format(log_dir))
    os.system("make kill")
    time.sleep(0.2)
    # parse
    with open(os.path.join(log_dir, "client.log")) as f:
        lines = f.readlines()
        if len(lines) > 5:
            (_, bct, block_size, _, _) = lines[4].split()
            bct = int(bct)
            block_size = int(block_size)
            data[0].append(bct)
            data[1].append(block_size)
            data[2].append(block_size / bct * 1000)
            i += 1
        else:
            continue
# print csv file
with open(os.path.join(test_dir, "results.csv"), 'w') as f:
    for i in range(len(data[0])):
        f.write("{},{},{}\n".format(data[0][i], data[1][i], data[2][i]))

# save the config
with open(os.path.join(test_dir, 'config.json'), 'w') as f:
    json.dump(config, f)
