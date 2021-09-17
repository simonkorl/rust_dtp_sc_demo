# WIP: Poster 版本 Rust 测试应用与脚本

## 新增功能

### test.py

利用脚本进行自动化测试。该脚本会通过 python 运行系统命令从而在本地进行测试（使用 loopback）。**不过目前的脚本每次只能运行一种测试，而且需要手动调节好实验环境，请一定注意！**

该程序从本目录下读取 ./config.json ，并且将运行结果打上时间戳输出在 ./results 目录下

运行的测试程序通过 aitrans\_server 底下的 run\_\* 脚本实现。server 部分的脚本使用了 taskset -c 0

### config.json

test.py 通过读取 config.json 从而实现测试，其结构如下：

```json
{
"cmd": "make test", //执行的系统命令。有三个可选值。 "make test" 代表运行 Rust 版本的测试程序，"make feature_test" 代表用 C 接口的同步 CC 测试程序，"make feature_multi_test",代表使用 C 接口的异步 CC 测试程序
"num": 10, // 运行实验的次数，也就是可以获得的数据的数量
"branch": "easytrans_full_speed", // 仅作为注释和提醒作用，代表当前使用的 DTP 分支。不同的 DTP 分支可以提供不同的测试能力，具体在下一小节说明。
"comment": "table 1 group 2 (full speed)" // 注释，提醒作用
}
```

### 测试用 DTP 分支

在 AItransDTP 中可以下载到若干个测试用分支，它们分别进行了如下的修改：

1. `poster_cc_period`: 关闭了 scheduler 和 FEC 的 C 接口，使得程序仅使用原始 DTP 的相关实现。（FEC 的值永远设置为 0）
2. `poster_cc_peroid_trigger`：使用 sleep 的方法，为同步和异步调用提供 period 的方法
2. `poster_easytrans`：仅关闭了 FEC 的 C 接口从而直接返回 0。会调用 scheduler 的 C 接口函数 SolutionSelectBlock 和 SolutionShouldDropBlock
3. `poster_easytrans_full_speed`：在 easytrans 分支的基础上将 Reno 的拥塞窗口调整为无穷大。

### 测试用 C 程序说明

这里指的是 `dtp_server/demo/solution.cxx** 程序。这个程序为了实验进行了大量的修改。主要的改动是修改了拥塞控制算法和调度器的实现，使得其可以不再利用 unordered_map 并且可以快速返回。经过测试，这会大幅提升运行速度。

**该 C 程序需要注意检查两点**：

1. 返回的 pacing_rate 是否为无穷大
2. 返回的 cwnd 是否为无穷大

请根据需要进行修改。

### 测试说明

根据本次 poster 的表格，以前得到数据的运行命令与分支组合见下表：

| 表格说明                     | 运行命令                  | DTP 分支                       |
| table 1 group 2              | make test                 | poster\_easytrans              |
| table 1 group 2 (full speed) | make test                 | poster\_easytrans\_full\_speed |
| table 1 group 3              | make feature\_test        | poster\_easytrans              |
| table 1 group 3 (full speed) | make feature\_test        | poster\_easytrans              |
| table 2 group 1              | make feature\_test        | poster\_cc\_period             |
| table 2 group 2              | make feature\_multi\_test | poster\_cc\_period             |
| table 2 group 3              | make feature\_test        | poster\_cc\_period\_trigger    |
| table 2 group 4              | make feature\_multi\_test | poster\_cc\_period\_trigger    |

其中，除了`table 1 group 3`的数据使用了 cwnd 作为 C 拥塞的返回值以外，其他的返回值均为无穷大。

## Deprecated: 原始测试程序说明

警告：该分支的程序**相当**不稳定，客户端与服务端的程序很可能有错误，请谨慎使用。

该分支希望用来测试 DTP 的运行效率以及接入 C 动态库之后的运行效率，从而得出模块化 DTP 的性能优化程度。

该分支希望使得 Rust 版本的测试应用可以发送和接收块的大小极大的数据块，从而测试整体上的发送速率。我们希望看到运行的过程中一个进程可以跑满一个 CPU 。

相对于基础版本而言增加了如下的特性：

1. 会在发送数据包之前发送一个记录“块”总数的数据块（int），客户端会记录这个数量并且在完成块的数量达到设定的数量的时候自动结束程序
2. 通过对 server 的修改，使得其每一次发送大小为 1,000,000B 的数据，最终可以发送 1,000,000 整数倍大小的数据块
  1. 然而这个功能还有不少的 Bug。最核心的问题之一就是它在传送巨大数据块（如 1G）的时候，client 端好像不能完全接收到，但是 server 端说已经发送出去了。这会导致 server 端在最后出现一个 30s 左右的 timeout 并且期间不会做任何事情，同时也无法接收到任何 ACK 数据包。这可能是需要解决的一个问题。
3. 对最后的输出 log 数据进行了检查和调整。

理论上具有以下的功能：

1. 可以利用 DTP 的 FFI 接口，与 C 实现的调度算法等算法进行联合测试【已验证可行】
2. 可以创建镜像并且用于运行得到实验数据【暂时没有测试过】
3. 可以使用 FEC 的功能并且进行测试【暂时没有测试过】

## 重要：Hard Code

为了实现巨大数据块的传输，一部分逻辑以非常简陋的 Hard Code 实现。请注意以下说明的若干地方：

1. server
  1. 481 行附近描述了在发送数据块之前先发送一个记录了当前 cfgs 里面数据块数量的块。
  2. 514 行附近的 while 循环中书写了每一次发送 1000000B 大小的数据，直到将全部数据块发送完毕为止。在 522 行附近检查了发送数据的长度是否和记录的块长度一致。请注意：这段代码逻辑没有任何的地方检查了**如果剩余数据不足 1000000B 如何进行发送**，所以最后只能发送 1000000 整数倍的数据。
  3. 527 行进行了发送数据总量的统计，现在发送数据总量按照服务端发送块的总大小来决定。
  4. 33 行附近增加了一个可以把 i32 转换为 [u8] 的一个函数
  5. 如果要进行本地的运行测试，则需要调节 155 行附近初始窗口大小。**不过这可能会导致一些 BUG，请谨慎使用。**
2. client
  1. 64 行附近添加了 i32 与 [u8] 相互转换的函数
3. demo/solution.cxx
可以发现 SolutionCCTrigger 部分把窗口设置得特别大，这是因为窗口如果不够大可能会导致整个程序无法运行。有的时候 Reno 也不太好使，也会导致发送到一半的时候卡住。解决方法还在研究当中。
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

1. 如果要进行 FEC 功能的测试，需要修改两个地方：
  1. 修改 `dtp_server/src/main.rs` 下的`config.set_redundancy_rate(f32)`中的值来说明需要进行荣誉编码的比例。如果打算启用 C 语言的调度算法或是拥塞控制，那么需要在 solution.cxx 中修改 `SolutionRedundancy` 函数的返回值来得到相同的结果。
  2. 修改 `dtp_server/src/main.rs` 下 383 行附近的 `conn.set_tail()` 中的数值。其中 tail 代表块的尾部数据，tail_size 代表块尾部数据的大小。通过 `conn.set_tail()** 来设定需要添加冗余的尾部数据大小
2. 现在的 server/client 只能因为**连接超时**的原因断开链接，**这可能会导致额外的测试时间开销**。需要进行检查并且进行一定的修改。
3. 现在采用的测试用镜像基本上会以 aitrans-server 目录为核心进行构建。可以参考 dockerfile 中的写法，只需要提供 server 和 client 的文件即可获得测试用的镜像

