IMAGE_NAME=simonkorl0228/test_image:yg.reno

all: server client

server:
	cd dtp_server && cargo build --release

server_debug:
	cd dtp_server && cargo build

server_interface: library
	cd dtp_server && RUSTFLAGS="-C link-args=-L$(CURDIR)/dtp_server/demo -lsolution" cargo build --release --features "interface"

server_interface_multi: library
	cd dtp_server && RUSTFLAGS="-C link-args=-L$(CURDIR)/dtp_server/demo -lsolution" cargo build --release --features "interface multi-thread-cc"

server_interface_debug: debug_library
	cd dtp_server && RUSTFLAGS="-C link-args=-L$(CURDIR)/dtp_server/demo -lsolution" cargo build --features "interface"

client:
	cd dtp_client && cargo build --release

client_debug:
	cd dtp_client && cargo build

debug: server_debug client_debug
	cp -rf dtp_server/target/debug/dtp_server aitrans-server/bin/server
	cp -rf dtp_client/target/debug/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

feature_debug: server_interface_debug client_debug
	cp -rf dtp_server/target/debug/dtp_server aitrans-server/bin/server
	cp -rf dtp_client/target/debug/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

test: server client
	cp -rf dtp_server/target/release/dtp_server aitrans-server/bin/server
	cp -rf dtp_client/target/release/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

feature_test: server_interface client
	cp dtp_server/target/release/dtp_server aitrans-server/bin/server
	cp dtp_client/target/release/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

feature_multi_test: server_interface_multi client
	cp dtp_server/target/release/dtp_server aitrans-server/bin/server
	cp dtp_client/target/release/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

build_interface: server_interface client

image_build:
	sudo docker build . -t $(IMAGE_NAME)

library: dtp_server/demo/solution.cxx dtp_server/demo/solution.hxx
	cd dtp_server/demo && g++ -shared -fPIC solution.cxx -I. -o libsolution.so && cp -f libsolution.so ../../aitrans-server/lib

debug_library: dtp_server/demo/solution.cxx dtp_server/demo/solution.hxx
	cd dtp_server/demo && g++ -shared -fPIC -g solution.cxx -I. -o libsolution.so && cp -f libsolution.so ../../aitrans-server/lib

kill:
	./aitrans-server/kill_server.sh

clean:
	cd dtp_client && cargo clean
	cd dtp_server && cargo clean
	cd dtp_utils && cargo clean
