# 具有初始窗口调节以及冗余程度调节 Rust 测试应用

警告：该分支的程序并不稳定，客户端与服务端的程序很可能有错误，请谨慎使用。

在基础的 Rust 测试应用之上有以下特性：

1. 通过 hard code 可以修改初始窗口的大小，进而间接调节初始发送速率（只能改变 reno 拥塞控制算法）
2. 通过 hard code 可以修改冗余编码的程度

**注意**：该版本需要一个特殊的 DTP 分支，这个分支仅用来做一些实验，所以会开放一些特殊的实验接口。

兼具以下的功能：

1. 可以利用 DTP 的 FFI 接口，与 C 实现的调度算法等算法进行联合测试
2. 可以创建镜像并且用于运行得到实验数据
3. 可以使用 FEC 的功能并且进行测试

## 重要：Hard Code 修改与说明

### 拥塞窗口相关

根据实验需求，可以通过 API 修改 reno 的初始拥塞窗口大小，从而达到比如说：“初始发送速率为 0.5MB/s”这样的需求。

在 dtp_server/src/main.rs:149 行附近有两个变量 `rtt` 和 `init_cwnd`·通过修改这两个变量即可修改初始窗口的大小。

这里使用了两个接口函数 `set_init_cwnd` 和 `set_init_pacing_rate`。这两个接口函数都是新添加的函数，并且后者在这里的含义是初始化 ssthresh 。

目前一系列的实验中默认：ssthresh 的初始值是 cwnd 的一半。**这会跳过慢启动阶段。**

### 冗余编码部分

在 dtp_server/src/main.rs:154 行附近有一系列的变量可以调整，后面的注释应该已经足够说明它们的含义。总体上想要表达的是：对最后的若干个包增加冗余，然后冗余包的数量为 `redun_pkt_num`

## Makefile 提供指令

* `make test`：在本地的 aitrans-server 目录下进行测试，可以得到四个运行结果文件，分别为 `aitrans-server/client.log`、`aitrans-server/client_err.log`、`aitrans-server/log/server.log`、`aitrans-server/server_err.log`
* `make feature_test`: 利用 dtp_server/demo 下的 cpp 代码编译形成动态链接库，将其拷贝到`aitrans-server/lib`目录下并且利用其运行使用 C 接口的服务端程序
* `make image_build`: 创建一个名称由`IMAGE_NAME`决定的镜像，该镜像可以配合 evaluate 目录下的脚本进行测试

## 测试应用与系统大体约定说明

### server

放置于 `aitrans-server/bin` 目录下。

使用方法类似`./bin/server 127.0.0.1 5555 'trace/block_trace/aitrans_block.txt'`，接受至少三个参数：IP,PORT和 TRACE 的路径

输出在 `./log/server_aitrans.log` 中，应具有一定格式的统计信息。（统计信息没有硬性要求）

### client

放置于 `aitrans-server` 目录下。

使用方法类似`./client 127.0.0.1 5555 --no-verify`，需要可以接受 `--no-verify` 参数。需要接受：IP，PORT 两个变量。

输出在 `aitrans-server` 目录下，`client.log` 应符合对应格式，并且最后应该提供具有一定格式的统计信息。（统计信息没有硬性要求）

## 其他说明

2. 现在的 server/client 只能因为**连接超时**的原因断开链接，**这可能会导致额外的测试时间开销**。需要进行检查并且进行一定的修改。断开的连接时间被设置为了 3s。
3. 现在采用的测试用镜像基本上会以 aitrans-server 目录为核心进行构建。可以参考 dockerfile 中的写法，只需要提供 server 和 client 的文件即可获得测试用的镜像

## 未来开发重点

- [ ] 读取配置文件来调节拥塞控制算法的参数

