#!/bin/bash
LD_LIBRARY_PATH=./lib RUST_BACKTRACE=1 ./bin/server 127.0.0.1 5555 trace/block_trace/aitrans_block.txt &> ./log/server_err.log &
