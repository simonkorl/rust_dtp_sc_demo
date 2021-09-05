# dtp_utils

dtp_utils 中包含一些可以处理 dtp_config 相关的操作函数。

该仓库将部分和 dtp_config 有关的 C 函数进行了封装，并提供一个 Rust 库文件使得 Rust 程序可以调用。对应的 C 的程序为 `include/dtp_config.h` 与 `src/dtp_config.c`，这两个程序可以直接复制并使用。

## dtp_config 基本概念

一个 dtp_config 是一个描述数据块的方式，包括数据块的大小、截止时间、优先级和发送时间间隔。利用 dtp_config 可以编写与 DTP 协议相关的测试程序。

dtp_config 的 C 定义如下：

```c
struct dtp_config {
    int deadline;   // ms
    int priority;   // 0 is the highest priority, larger the number lower the priority
    int block_size; // byte
    float send_time_gap;// s
};

typedef struct dtp_config dtp_config;
```

一个 dtp_config 可以在文件中使用空格作为分割符进行书写并且调用对应函数解析，一个示例 dtp_config 如下：

```txt
0.01 100 1000 1
```

其含义为：在上一条配置之后的 0.01s 后发送一个大小为 1000B 的数据块，该数据块具有 100ms 的截止时间以及 1 的优先级。

请注意：dtp_config 只提供用于储存的数据结构，但是并不提供发送的功能和逻辑。所有有关 dtp_config 字段的解释以及对应功能的实现需要自行完成。

## 提供的函数

* `fn get_current_usec()`: 调用 `gettimeofday` 获得当前时刻对应的微秒数。
* `fn parse_dtp_config()`: 解析目标文件名下的 dtp_config 并且返回 Vec<dtp_config> ，不需要担心内存问题。请注意：对应的C函数需要手动释放空间！
