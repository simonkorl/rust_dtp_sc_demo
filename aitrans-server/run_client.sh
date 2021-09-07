#!/bin/bash
LD_LIBRARY_PATH=./lib RUST_LOG=info ./client 127.0.0.1 5555 --no-verify &> client_err.log &
