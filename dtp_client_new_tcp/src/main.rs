// Copyright (C) 2018-2019, Cloudflare, Inc.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
//     * Redistributions of source code must retain the above copyright notice,
//       this list of conditions and the following disclaimer.
//
//     * Redistributions in binary form must reproduce the above copyright
//       notice, this list of conditions and the following disclaimer in the
//       documentation and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
// IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
// THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
// PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
// CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
// EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
// PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
// LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
// NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#[macro_use]
extern crate log;
use std::net::ToSocketAddrs;

use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use quiche::Block;

use ring::rand::*;

use dtp_utils::get_current_usec;
use streamparser::StreamParser;

// use std::time::{
//     Instant,
//     SystemTime,
//     UNIX_EPOCH,
// };
extern crate time;

const MAX_DATAGRAM_SIZE: usize = 1350;

const HTTP_REQ_STREAM_ID: u64 = 4;

const USAGE: &str = "Usage:
  client [options] ADDR PORT
  client -h | --help

Options:
  --wire-version VERSION   The version number to send to the server [default: babababa].
  --dump-packets PATH      Dump the incoming packets as files in the given directory.
  --no-verify              Don't verify server's certificate.
  --cc-algorithm NAME      Set client congestion control algorithm [default: reno].
  -h --help                Show this screen.
";
// other option
//   --max-data BYTES         Connection-wide flow control limit [default:
// 10000000].   --max-stream-data BYTES  Per-stream flow control limit [default:
// 1000000].

#[derive(Clone, Debug, Default, Copy)]
#[repr(C)]
struct BlockInfo {
    start_timestamp: u64,
    end_timestamp: u64,
    bct: u64,
    deadline: i32,
    priority: i32,
    block_size: i32,
    id: u64
}


fn main() {
    let path = Path::new("client.log");
    let log_path = Path::new("client.csv");
    let display = path.display();
    // Open a file in write-only mode, returns `io::Result<File>`
    let mut file = match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", display, why),
        Ok(file) => file,
    };
    let mut log_file = match File::create(&log_path) {
        Err(why) => panic!("couldn't create {}: {}", log_path.display(), why),
        Ok(file) => file,
    };
    // let (bct_offset, s) = ntp_offset("ntp1.aliyun.com:123");
    let bct_offset = 0; // docker environment not need ntp
    // let (bct_offset, s) = ntp_offset("0.pool.ntp.org:123");
    // let (bct_offset, s) = ntp_offset("time1.cloud.tencent.com:123");
    // match file.write_all(s.as_bytes()) {
    //     Err(why) => panic!("couldn't write to {}: {}", display, why),
    //     _ => (),
    // }

    let mut buf = [0; 65535];
    let mut out = [0; MAX_DATAGRAM_SIZE];

    env_logger::builder()
        .default_format_timestamp_nanos(true)
        .init();

    let args = docopt::Docopt::new(USAGE)
        .and_then(|dopt| dopt.parse())
        .unwrap_or_else(|e| e.exit());

    // let max_data = args.get_str("--max-data");
    let max_data = "10000000000";
    let max_data = u64::from_str_radix(max_data, 10).unwrap();

    // let max_stream_data = args.get_str("--max-stream-data");
    let max_stream_data = "10000000000";
    let max_stream_data = u64::from_str_radix(max_stream_data, 10).unwrap();

    let version = args.get_str("--wire-version");
    let version = u32::from_str_radix(version, 16).unwrap();

    let dump_path = if args.get_str("--dump-packets") != "" {
        Some(args.get_str("--dump-packets"))
    } else {
        None
    };

    let url_string = format!("http://{0}:{1}", args.get_str("ADDR"), args.get_str("PORT"));
    let url = url::Url::parse(&url_string).unwrap();

    // Setup the event loop.
    let poll = mio::Poll::new().unwrap();
    let mut events = mio::Events::with_capacity(1024);

    // Resolve server address.
    let peer_addr = url.to_socket_addrs().unwrap().next().unwrap();

    // Bind to INADDR_ANY or IN6ADDR_ANY depending on the IP family of the
    // server address. This is needed on macOS and BSD variants that don't
    // support binding to IN6ADDR_ANY for both v4 and v6.
    let bind_addr = match peer_addr {
        std::net::SocketAddr::V4(_) => "0.0.0.0:0",
        std::net::SocketAddr::V6(_) => "[::]:0",
    };

    // println!("peer_addr = {}", peer_addr);
    let s = format!("peer_addr = {}\n", peer_addr);
    match file.write_all(s.as_bytes()) {
        Err(why) => panic!("couldn't write to {}: {}", display, why),
        _ => (),
    }

    // Create the UDP socket backing the QUIC connection, and register it with
    // the event loop.
    let socket = std::net::UdpSocket::bind(bind_addr).unwrap();
    socket.connect(peer_addr).unwrap();

    let socket = mio::net::UdpSocket::from_socket(socket).unwrap();
    poll.register(
        &socket,
        mio::Token(0),
        mio::Ready::readable(),
        mio::PollOpt::edge(),
    )
    .unwrap();

    // Create the configuration for the QUIC connection.
    let mut config = quiche::Config::new(version).unwrap();

    config.verify_peer(true);

    config
        .set_application_protos(b"\x05hq-25\x05hq-24\x05hq-23\x08http/0.9")
        .unwrap();

    config.set_max_idle_timeout(5000);
    // config.set_max_packet_size(MAX_DATAGRAM_SIZE as u64);
    config.set_initial_max_data(max_data);
    config.set_initial_max_stream_data_bidi_local(max_stream_data);
    config.set_initial_max_stream_data_bidi_remote(max_stream_data);
    config.set_initial_max_streams_bidi(1000000);
    // config.set_initial_max_streams_uni(1000000);
    // config.set_disable_active_migration(true);

    if args.get_bool("--no-verify") {
        config.verify_peer(false);
    }

    if std::env::var_os("SSLKEYLOGFILE").is_some() {
        config.log_keys();
    }

    config
        .set_cc_algorithm_name(args.get_str("--cc-algorithm"))
        .unwrap();

    // Generate a random source connection ID for the connection.
    let mut scid = [0; 16];
    SystemRandom::new().fill(&mut scid[..]).unwrap();
    let scid = quiche::ConnectionId::from_ref(&scid);

    let server_name = "new-dtp-server";

    let to = format!("{}:{}", args.get_str("ADDR"), args.get_str("PORT")).parse().unwrap();

    // Create a QUIC connection and initiate handshake.
    let mut conn = quiche::connect(Some(&server_name), &scid, to, &mut config).unwrap();

    info!(
        "connecting to {:} from {:} with scid {}",
        peer_addr,
        socket.local_addr().unwrap(),
        hex_dump(&scid)
    );

    let (write, send_info) = conn.send(&mut out).expect("initial send failed");

    while let Err(e) = socket.send_to(&out[..write], &send_info.to) {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            debug!("send() would block");
            continue;
        }

        panic!("send() failed: {:?}", e);
    }

    debug!("written {}", write);

    // let req_start = std::time::Instant::now();

    let mut req_sent = false;

    let mut pkt_count = 0;

    // stats for QoS and QoE
    let mut recv_bytes: u64 = 0; // Total bytes the client receives
    let mut complete_bytes: u64 = 0; // Total bytes of complete blocks
    let mut good_bytes: u64 = 0; // Total bytes of blocks that arrive before deadline

    let mut block_vec: Vec<BlockInfo> = Vec::new();
    let mut parser = StreamParser::new(65535);

    let s =
        format!("test begin!\n\nblockid\tbct\tsize\tpriority\ndeadline\n");
    match file.write_all(s.as_bytes()) {
        Err(why) => panic!("couldn't write to {}: {}", display, why),
        _ => (),
    }
    let s = 
        format!("block_id,bct,size,priority,deadline,duration\n");
    match log_file.write_all(s.as_bytes()) {
        Err(why) => panic!("couldn't write to {}: {}", display, why),
        _ => (),
    }
    let start_timestamp = std::time::Instant::now();
    loop {
        poll.poll(&mut events, conn.timeout()).unwrap();

        // Read incoming UDP packets from the socket and feed them to quiche,
        // until there are no more packets to read.
        'read: loop {
            // If the event loop reported no events, it means that the timeout
            // has expired, so handle it without attempting to read packets. We
            // will then proceed with the send loop.
            if events.is_empty() {
                debug!("timed out");

                conn.on_timeout();
                break 'read;
            }

            let (len, from) = match socket.recv_from(&mut buf) {
                Ok(v) => v,

                Err(e) => {
                    // There are no more UDP packets to read, so end the read
                    // loop.
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("recv() would block");
                        break 'read;
                    }

                    panic!("recv() failed: {:?}", e);
                },
            };

            let recv_info = quiche::RecvInfo { from };

            debug!("got {} bytes", len);
            recv_bytes += len as u64;

            if let Some(target_path) = dump_path {
                let path = format!("{}/{}.pkt", target_path, pkt_count);

                if let Ok(f) = std::fs::File::create(&path) {
                    let mut f = std::io::BufWriter::new(f);
                    f.write_all(&buf[..len]).ok();
                }
            }

            pkt_count += 1;

            // Process potentially coalesced packets.
            let read = match conn.recv(&mut buf[..len], recv_info) {
                Ok(v) => v,

                Err(quiche::Error::Done) => {
                    debug!("done reading");
                    break;
                },

                Err(e) => {
                    error!("recv failed: {:?}", e);
                    break 'read;
                },
            };

            debug!("processed {} bytes", read);
        }

        // Send an HTTP request as soon as the connection is established.
        if conn.is_established() && !req_sent {
            info!("sending HTTP request for {}", url.path());

            let req = format!("GET {}\r\n", url.path());
            conn.stream_send(HTTP_REQ_STREAM_ID, req.as_bytes(), true)
                .unwrap();

            req_sent = true;
        }

        // Process all readable streams.
        for s in conn.readable() {
            while let Ok((read, fin)) = conn.stream_recv(s, &mut buf) {
                debug!("received {} bytes", read);

                let stream_buf = &buf[..read];

                debug!(
                    "stream {} has {} bytes (fin? {})",
                    s,
                    stream_buf.len(),
                    fin
                );

                let mut total_size = 0;
                let mut blocks: Vec<BlockInfo> = Vec::new();
                
                while total_size < read {
                    total_size += parser.recv(&buf[total_size..read], read - total_size);
                    blocks.append(&mut parser.consume());
                }

                for block in blocks.iter() {
                    // Log into client.log
                    // BlockID bct BlockSize Priority Deadline
                    let s = format!("{:<10}\t{:10}\t{:10}\t{:10}\t{:10}\n", 
                        block.id, 
                        block.bct, 
                        block.block_size, 
                        block.priority, 
                        block.deadline
                    );
                    match file.write_all(s.as_bytes()) {
                        Err(why) =>
                        panic!("couldn't write to {}: {}", display, why),
                        _ => (),
                    }

                    let s = format!("{},{},{},{},{},{}\n", 
                        block.id, 
                        block.bct, 
                        block.block_size, 
                        block.priority, 
                        block.deadline,
                        start_timestamp.elapsed().as_micros()
                    );
                    match log_file.write_all(s.as_bytes()) {
                        Err(why) =>
                        panic!("couldn't write to {}: {}", log_path.display(), why),
                        _ => (),
                    }

                    complete_bytes += block.block_size as u64;

                    if block.bct < block.deadline as u64 {
                        good_bytes += block.block_size as u64;
                    }
                }
                block_vec.append(&mut blocks);

                // print!("{}", unsafe {
                //     std::str::from_utf8_unchecked(&stream_buf)
                // });

                // // The server reported that it has no more data to send,
                // which // we got the full response. Close the
                // connection. if s == HTTP_REQ_STREAM_ID && fin
                // {     info!(
                //         "response received in {:?}, closing...",
                //         req_start.elapsed()
                //     );

                //     conn.close(true, 0x00, b"kthxbye").unwrap();
                // }
            }
        }

        // Generate outgoing QUIC packets and send them on the UDP socket, until
        // quiche reports that there are no more packets to be sent.
        loop {
            let (write, send_info) = match conn.send(&mut out) {
                Ok(v) => v,

                Err(quiche::Error::Done) => {
                    debug!("done writing");
                    break;
                },

                Err(e) => {
                    error!("send failed: {:?}", e);

                    conn.close(false, 0x1, b"fail").ok();
                    break;
                },
            };

            if let Err(e) = socket.send_to(&out[..write], &send_info.to) {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    debug!("send() would block");
                    break;
                }

                panic!("send() failed: {:?}", e);
            }

            debug!("written {}", write);
        }

        if conn.is_closed() {
            debug!("connection is closed");
            let s = format!("connection closed, {:?}, total_bytes={}, complete_bytes={}, good_bytes={}, total_time={}\n",
                            conn.stats(),
                            recv_bytes,
                            complete_bytes,
                            good_bytes,
                            start_timestamp.elapsed().as_micros());
            match file.write_all(s.as_bytes()) {
                Err(why) => panic!("couldn't write to {}: {}", display, why),
                _ => (),
            }
            // println!("connection closed, {:?}", conn.stats());
            break;
        }
    }
}

fn hex_dump(buf: &[u8]) -> String {
    let vec: Vec<String> = buf.iter().map(|b| format!("{:02x}", b)).collect();

    vec.join("")
}

// /// use nums of ntp_single_offset and compute mean of this offsets, to get a
// /// stabler offset.
// fn ntp_offset(server_address: &str) -> (i64, String) {
//     let mut ntp_nums = 20;
//     let mut sum_offset = 0;
//     for _ in 0..ntp_nums {
//         let offset = ntp_single_offset(server_address);
//         if offset == i64::max_value() {
//             ntp_nums -= 1;
//         } else {
//             sum_offset += offset;
//         }
//     }
//     if ntp_nums == 0 {
//         let s = format!("get ntp offset failed\n");
//         return (0, s);
//     } else {
//         let s = format!(
//             "get ntp offset success, ntp nums:{}, offset:{}\n",
//             ntp_nums,
//             sum_offset / ntp_nums
//         );
//         (sum_offset / ntp_nums, s)
//     }
// }

// /// get offset between local time and server time. if failed, return
// /// i64::max_value().
// fn ntp_single_offset(server_address: &str) -> i64 {
//     let t1 = Instant::now();
//     let response = match ntp::request(server_address) {
//         Ok(v) => v,
//         _ => return i64::max_value(),
//     };
//     let t2 = Instant::now();

//     let start = SystemTime::now();
//     let since_the_epoch = start
//         .duration_since(UNIX_EPOCH)
//         .expect("Time went backwards");
//     let local_sec = since_the_epoch.as_secs();
//     let local_nsec = since_the_epoch.subsec_nanos();

//     let ntp_time = response.transmit_time;
//     let ntp_time: time::Timespec = time::Timespec::from(ntp_time);
//     let offset = (local_sec as i64 - ntp_time.sec) * 1000 +
//         (local_nsec as i64 - ntp_time.nsec as i64) / 1000_000;
//     return offset - t2.duration_since(t1).as_millis() as i64 / 2;
// }
mod loopbytes;
mod streamparser;