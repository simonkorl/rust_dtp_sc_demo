# dtp_server: Rust 版本测试程序服务端

可以与 Rust 版本的测试程序客户端共同组成一组测试程序来获得测试数据结果。

该项目依赖 DTP 仓库进行实现，DTP 仓库需要支持 `interface` 特性。可以通过修改`Cargo.toml`来调整读取 DTP 仓库的位置。

该项目依赖 dtp_utils 进行文件的解析，可以通过配置 `Cargo.toml` 修改 dtp_utils 仓库的位置。 

## 一般使用方法

使用 `cargo build --release` 编译得到可执行文件 `dtp_server`，将其复制到合适的位置，并且运行类似下面的代码进行运行

```bash
#!/bin/bash
LD_LIBRARY_PATH=./lib \ # 如果启用 interface 特性则必须提供 C 的动态链接库
RUST_BACKTRACE=1 \ # 如果需要回调信息则修改该变量
RUST_LOG=debug \ # 如果需要调试信息则修改该变量
./bin/server 127.0.0.1 \ # 监听的 IP 地址
5555 \ # 监听的端口号
'trace/block_trace/aitrans_block.txt' \ # 输入的文件目录。由 dtp_config 格式组成
&> ./log/server_err.log &
```

## 输出

该程序会在 `./log/server_aitrans.log` 目录下生成一份简单的报告文件，调试信息主要集中在 stderr 中。

请保证 `./log` 目录存在，否则程序可能会报错。

## interface 特性

该项目可以启用 `interface` 特性，这会启用 DTP 的 `interface` 特性来提供 C 的 FFI 。通过给予一个动态链接库，该程序就可以得到不同的运行效果。

在 demo 目录下具有可以编译成动态链接库的 cpp 程序。这两个文件会在启用 `interface` 后作为编译时的库文件参考来使用。在编译完成后，通过使用 `LD_LIBRARY_PATH` 来指定连接的动态链接库的目录来连接名为 `libsolution.so` 的动态链接库。

为了生成这样的动态链接库，你可以使用类似下面的代码进行编译：

```sh
g++ -shared -fPIC solution.cxx -I. -o libsolution.so
```

## 其他说明

1. 该程序默认使用 reno 作为拥塞控制算法。如果启用 `interface` 特性则会采用 `cc_trigger`。这一部分在 159 行附近。
2. server 的运行需要 cert.crt 与 cert.key 。按照 aitrans-server 目录下的文件结构一般不会出现问题。

