all: server client

server:
	cd dtp_server && cargo build --release

server_interface: library
	cd dtp_server && RUSTFLAGS="-C link-args=-L$(CURDIR)/dtp_server/demo -lsolution" cargo build --release --features "interface"

client:
	cd dtp_client && cargo build --release

test: server client
	cp dtp_server/target/release/dtp_server aitrans-server/bin/server
	cp dtp_client/target/release/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

feature_test: server_interface client
	cp dtp_server/target/release/dtp_server aitrans-server/bin/server
	cp dtp_client/target/release/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh

build_interface: server_interface client

image_build:
	sudo docker build . -t simonkorl0228/test_image:yg.reno

library: dtp_server/demo/solution.cxx dtp_server/demo/solution.hxx
	cd dtp_server/demo && g++ -shared -fPIC solution.cxx -I. -o libsolution.so
	cp -rf dtp_server/demo/libsolution.so aitrans-server/lib
kill:
	./aitrans-server/kill_server.sh

clean:
	cd dtp_client && cargo clean
	cd dtp_server && cargo clean
	cd dtp_utils && cargo clean
