server: 
	cd dtp_server && cargo build
client:
	cd dtp_client && cargo build
test: server client
	cp dtp_server/target/debug/dtp_server aitrans-server/bin/server
	cp dtp_client/target/debug/dtp_client aitrans-server/client
	cd aitrans-server && ./run_server.sh
	sleep 0.1
	cd aitrans-server && ./run_client.sh
kill:
	./aitrans-server/kill_server.sh