# 基础 Rust 测试应用

警告：该分支的程序并不稳定，客户端与服务端的程序很可能有错误，请谨慎使用。

具有以下的功能：

1. 可以利用 DTP 的 FFI 接口，与 C 实现的调度算法等算法进行联合测试
2. 可以创建镜像并且用于运行得到实验数据
3. 可以使用 FEC 的功能并且进行测试

## Makefile 提供指令

* `make test`：在本地的 aitrans-server 目录下进行测试，可以得到四个运行结果文件，分别为 `aitrans-server/client.log`、`aitrans-server/client_err.log`、`aitrans-server/log/server.log`、`aitrans-server/server_err.log`
* `make feature_test`: 利用 dtp_server/demo 下的 cpp 代码编译形成动态链接库，将其拷贝到`aitrans-server/lib`目录下并且利用其运行使用 C 接口的服务端程序
* `make image_build`: 创建一个名称由`IMAGE_NAME`决定的镜像，该镜像可以配合 evaluate 目录下的脚本进行测试

## 其他说明

1. 如果要进行 FEC 功能的测试，需要修改两个地方：
  1. 修改 `dtp_server/src/main.rs` 下的`config.set_redundancy_rate(f32)`中的值来说明需要进行荣誉编码的比例。如果打算启用 C 语言的调度算法或是拥塞控制，那么需要在 solution.cxx 中修改 `SolutionRedundancy` 函数的返回值来得到相同的结果。
  2. 修改 `dtp_server/src/main.rs` 下 383 行附近的 `conn.set_tail()` 中的数值。其中 tail 代表块的尾部数据，tail_size 代表块尾部数据的大小。通过 `conn.set_tail()** 来设定需要添加冗余的尾部数据大小
2. 现在的 server/client 只能因为**连接超时**的原因断开链接，**这可能会导致额外的测试时间开销**。需要进行检查并且进行一定的修改。

