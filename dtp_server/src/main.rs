#[macro_use]
extern crate log;

use std::net;
use std::net::SocketAddr;
use std::io::prelude::*;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use ring::rand::*;

use dtp_utils::*;

use std::fs::File;
use std::path::Path;

use mio::{Token, Poll, Waker, event::*};
use mio::net::{UdpSocket};

macro_rules! log {
    ($file:expr, $display:expr, $($arg:tt)*) => {
        let s = format!($($arg)*);
        match $file.write_all(s.as_bytes()) {
            Err(why) => panic!("couldn't write to {}: {}", $display, why),
            _ => (),
        };
    }
}

pub fn i32tou8(i: i32) -> [u8; 4] {
    unsafe {
        let i32_ptr: *const i32 = &i as *const i32;
        let u8_ptr: *const u8 = i32_ptr as *const u8;
        return [*u8_ptr.offset(0), *u8_ptr.offset(1), *u8_ptr.offset(2), *u8_ptr.offset(3)];
    }
}
const USAGE: &str = "Usage:
  server [options] ADDR PORT CONFIG
  server -h | --help

Options:
  -h --help                Show this screen.
  --dump-packets PATH      Dump the incoming packets as files in the given directory.
  --cc-algorithm NAME      Set client congestion control algorithm [default: reno].
";

const MAX_DATAGRAM_SIZE: usize = 1350;
const TIMEOUT: u64 = 5000;
const MAX_BLOCK_SIZE: usize = 1000000000;
static mut DATA_BUF: [u8; 1_000_100] = [0; 1_000_100];

struct ParticalSent {
    block_index: usize,

    block_size: usize,

    written: usize,
}

struct Client {
    conn: std::pin::Pin<Box<quiche::Connection>>,

    partial_responses: HashMap<u64, ParticalSent>,

    start_time: Option<u64>,

    end_time: Option<u64>,

    next_timeout: Option<Instant>,

    dtp_config_offset: usize,

    _token: Option<mio::Token>
}

type ClientMap = HashMap<Vec<u8>, (net::SocketAddr, Client)>;

fn main() -> Result<(), Box<dyn Error>> {
    let mut buf = [0; 65535];
    let mut out = [0; MAX_DATAGRAM_SIZE];
    env_logger::builder()
        .default_format_timestamp_nanos(true)
        .init();


    let args = docopt::Docopt::new(USAGE)
        .and_then(|dopt| dopt.parse())
        .unwrap_or_else(|e| e.exit());

    // Prepare random data
    let rng = SystemRandom::new();
    unsafe {
        rng.fill(&mut DATA_BUF).unwrap();
    }

    // load dtp configs
    let config_file = args.get_str("CONFIG");
    let cfgs = get_dtp_config(config_file);
    if cfgs.len() <= 0 {
        eprintln!("Error dtp config length: 0");
        panic!("Error: No dpt config is found");
    }

    // open Aitrans Log
    let path = Path::new("./log/server_aitrans.log");
    let display = path.display();

    // Open a file in write-only mode, returns `io::Result<File>`
    let mut file = match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", display, why),
        Ok(file) => file,
    };

    let dump_path = if args.get_str("--dump-packets") != "" {
        Some(args.get_str("--dump-packets"))
    } else {
        None
    };

    // let max_data = args.get_str("--max-data");
    let max_data = "10000000000";
    let max_data = u64::from_str_radix(max_data, 10).unwrap();

    // let max_stream_data = args.get_str("--max-stream-data");
    let max_stream_data = "10000000000";
    let max_stream_data = u64::from_str_radix(max_stream_data, 10).unwrap();

    // Setup the event loop.
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(1024);

    // Create the configuration for the QUIC connections.
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();

    config
        .load_cert_chain_from_pem_file("cert.crt")
        .unwrap();
    config
        .load_priv_key_from_pem_file("cert.key")
        .unwrap();

    config
        .set_application_protos(b"\x05hq-25\x05hq-24\x05hq-23\x08http/0.9")
        .unwrap();

    config.set_max_idle_timeout(TIMEOUT);
    config.set_max_packet_size(MAX_DATAGRAM_SIZE as u64);
    config.set_initial_max_data(max_data);
    config.set_initial_max_stream_data_bidi_local(max_stream_data);
    config.set_initial_max_stream_data_bidi_remote(max_stream_data);
    config.set_initial_max_streams_bidi(10000);
    // config.set_redundancy_rate(0.5);
    // config.set_init_cwnd(1_000_000u64);
    // config.set_init_pacing_rate(5_0000_0000);
    // config.set_initial_max_streams_uni(10000);
    config.set_disable_active_migration(true);

    if std::env::var_os("SSLKEYLOGFILE").is_some() {
        config.log_keys();
    }

    // config
    //     .set_cc_algorithm_name(args.get_str("--cc-algorithm"))
    //     .unwrap();
    if cfg!(feature="interface") {
        config.set_cc_algorithm_name("cc_trigger")?;
    } else {
        config.set_cc_algorithm_name("reno")?;
    }

    let rng = SystemRandom::new();
    let conn_id_seed =
        ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &rng).unwrap();

    // Create the UDP listening socket, and register it with the event loop.
    let mut socket = mio::net::UdpSocket::bind(format!("{}:{}", args.get_str("ADDR"), args.get_str("PORT")).parse()?)?;

    poll.registry().register(
        &mut socket,
        mio::Token(0),
        mio::Interest::READABLE
    )?;

    // Initial variables
    let mut clients = ClientMap::new();

    let mut pkt_count = 0;
    let mut start_timestamp: Option<u64> = None;
    let mut total_bytes = 0;
    let mut end_timstamp: Option<u64> = None;
    let mut sent_block_nums: bool  = false;
    // let mut good_bytes = 0;
    // let mut complete_bytes = 0;
    eprintln!("server start, timestamp: {}", get_current_usec());
    log!(file, display, "Begin DTP baseline server\nserver start, timestamp: {}\n", get_current_usec());
    'outer: loop {
        // Find the shorter timeout from all the active connections.
        //
        // TODO: use event loop that properly supports timers
        let conn_timeout =
            clients.values().filter_map(|(_, c)| c.conn.timeout()).min();

        // get sender timeout
        let sender_timeout = clients.values().filter_map(|(_, c)| c.next_timeout).min();
        debug!("sender timeout raw: {:?}", sender_timeout);
        let now = Instant::now();
        let sender_timeout = if sender_timeout.is_none() {
            None
        } else if sender_timeout.unwrap() <= now {
            Some(Duration::from_secs(0))
        } else {
            Some(sender_timeout.unwrap() - now)
        };

        let timeout = if conn_timeout.is_none() && sender_timeout.is_none() {
            debug!("None timeout");
            None
        } else if conn_timeout.is_none() {
            debug!("sender timeout!");
            sender_timeout
        } else if sender_timeout.is_none() {
            debug!("conn timeout!!");
            conn_timeout
        } else {
            let t = std::cmp::min(sender_timeout, conn_timeout);
            if t == sender_timeout {
                debug!("cmp: sender timeout!");
            } else {
                debug!("cmp: conn timeout!!");
            }
            t
        };
        debug!("timeout: {:?}", timeout);

        poll.poll(&mut events, timeout).unwrap();

        // Read incoming UDP packets from the socket and feed them to quiche,
        // until there are no more packets to read.
        'read: loop {
            // If the event loop reported no events, it means that the timeout
            // has expired, so handle it without attempting to read packets. We
            // will then proceed with the send loop.
            if events.is_empty() {
                debug!("timed out");

                clients.values_mut().for_each(|(_, c)| c.conn.on_timeout());

                break 'read;
            }

            let (len, src) = match socket.recv_from(&mut buf) {
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

            debug!("got {} bytes", len);

            let pkt_buf = &mut buf[..len];

            if let Some(target_path) = dump_path {
                let path = format!("{}/{}.pkt", target_path, pkt_count);

                if let Ok(f) = std::fs::File::create(&path) {
                    let mut f = std::io::BufWriter::new(f);
                    f.write_all(pkt_buf).ok();
                }
            }

            pkt_count += 1;

            // Parse the QUIC packet's header.
            let hdr = match quiche::Header::from_slice(
                pkt_buf,
                quiche::MAX_CONN_ID_LEN,
            ) {
                Ok(v) => v,

                Err(e) => {
                    error!("Parsing packet header failed: {:?}", e);
                    continue;
                },
            };

            trace!("got packet {:?}", hdr);

            let conn_id = ring::hmac::sign(&conn_id_seed, &hdr.dcid);
            let conn_id = &conn_id.as_ref()[..quiche::MAX_CONN_ID_LEN];

            // Lookup a connection based on the packet's connection ID. If there
            // is no connection matching, create a new one.
            let (_, client) = if !clients.contains_key(&hdr.dcid) &&
                !clients.contains_key(conn_id)
            {
                if hdr.ty != quiche::Type::Initial {
                    error!("Packet is not Initial");
                    continue;
                }

                if !quiche::version_is_supported(hdr.version) {
                    warn!("Doing version negotiation");

                    let len =
                        quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut out)
                            .unwrap();

                    let out = &out[..len];

                    if let Err(e) = socket.send_to(out, src) {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            debug!("send() would block");
                            break;
                        }

                        panic!("send() failed: {:?}", e);
                    }
                    continue;
                }

                let mut scid = [0; quiche::MAX_CONN_ID_LEN];
                scid.copy_from_slice(&conn_id);

                let mut odcid = None;

                if !args.get_bool("--no-retry") {
                    // Token is always present in Initial packets.
                    let token = hdr.token.as_ref().unwrap();

                    // Do stateless retry if the client didn't send a token.
                    if token.is_empty() {
                        warn!("Doing stateless retry");

                        let new_token = mint_token(&hdr, &src);

                        let len = quiche::retry(
                            &hdr.scid, &hdr.dcid, &scid, &new_token, &mut out,
                        )
                        .unwrap();

                        let out = &out[..len];

                        if let Err(e) = socket.send_to(out, src) {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                debug!("send() would block");
                                break;
                            }

                            panic!("send() failed: {:?}", e);
                        }
                        continue;
                    }

                    odcid = validate_token(&src, token);

                    // The token was not valid, meaning the retry failed, so
                    // drop the packet.
                    if odcid == None {
                        error!("Invalid address validation token");
                        continue;
                    }

                    if scid.len() != hdr.dcid.len() {
                        error!("Invalid destination connection ID");
                        continue;
                    }

                    // Reuse the source connection ID we sent in the Retry
                    // packet, instead of changing it again.
                    scid.copy_from_slice(&hdr.dcid);
                }

                debug!(
                    "New connection: dcid={} scid={}",
                    hex_dump(&hdr.dcid),
                    hex_dump(&scid)
                );

                let mut conn = quiche::accept(&scid, odcid, &mut config).unwrap();
                // conn.set_tail(5000); // ! if you enable redundancy without setting the tail, the program may panic in a unreachable branch
                let client = Client {
                    conn,
                    partial_responses: HashMap::new(),
                    start_time: Some(get_current_usec()),
                    end_time: None,
                    next_timeout: None,
                    dtp_config_offset: 0,
                    _token: Some(mio::Token(clients.len() + 100))
                };

                clients.insert(scid.to_vec(), (src, client));

                if start_timestamp.is_none() {
                    // Indicate the server is started by establishing the first connection
                    start_timestamp = Some(get_current_usec());
                }

                clients.get_mut(&scid[..]).unwrap()
            } else {
                match clients.get_mut(&hdr.dcid) {
                    Some(v) => v,

                    None => clients.get_mut(conn_id).unwrap(),
                }
            };

            // Process potentially coalesced packets.
            let read = match client.conn.recv(pkt_buf) {
                Ok(v) => v,

                Err(quiche::Error::Done) => {
                    debug!("{} done reading", client.conn.trace_id());
                    break;
                },

                Err(e) => {
                    error!("{} recv failed: {:?}", client.conn.trace_id(), e);
                    break 'read;
                },
            };

            debug!("{} processed {} bytes", client.conn.trace_id(), read);

            // if client.conn.is_in_early_data() || client.conn.is_established() {
            //     // Handle writable streams.
            //     for stream_id in client.conn.writable() {
            //         handle_writable(client, stream_id);
            //     }

            //     // Process all readable streams.
            //     for s in client.conn.readable() {
            //         while let Ok((read, fin)) =
            //             client.conn.stream_recv(s, &mut buf)
            //         {
            //             debug!(
            //                 "{} received {} bytes",
            //                 client.conn.trace_id(),
            //                 read
            //             );

            //             let stream_buf = &buf[..read];

            //             debug!(
            //                 "{} stream {} has {} bytes (fin? {})",
            //                 client.conn.trace_id(),
            //                 s,
            //                 stream_buf.len(),
            //                 fin
            //             );

            //             handle_stream(
            //                 client,
            //                 s,
            //                 stream_buf,
            //                 args.get_str("--root"),
            //             );
            //         }
            //     }
            // }
        }

        // Sending blocks
        for (_, client) in clients.values_mut() {
            if client.dtp_config_offset >= cfgs.len() {
                // Stop sending
                debug!("Stop sending");
                // match client.conn.close(true, quiche::Error::Done as u32 as u64, b"send done") {
                //     Ok(()) => continue,
                //     Err(quiche::Error::Done) => continue,
                //     Err(err) => panic!("{:?}", err),
                // }
            } else if client.dtp_config_offset == 0 && !sent_block_nums && client.conn.is_established(){
                debug!("Send block number block");
                client.next_timeout = None;
                let mut len = 0;
                len = match client.conn.stream_send_full(1, &i32tou8(cfgs.len() as i32), true, 100, 0, 1){
                    Ok(l) => l,
                    Err(err) => panic!("{:?}", err)
                };
                if len != 4 {
                    panic!("sent block number failed");
                }
                sent_block_nums = true;
            } else {
                // Send blocks
                if client.conn.is_established() {
                    if client.next_timeout.is_none() || client.next_timeout <= Some(Instant::now()) {
                        debug!("Send block {}", client.dtp_config_offset);
                        client.next_timeout = None;
                        for _ in client.dtp_config_offset..cfgs.len() {
                            // Send a block
                            let block = cfgs[client.dtp_config_offset];
                            let send_time_gap = block.send_time_gap;
                            let deadline = block.deadline as u64;
                            let priority = block.priority as u64;
                            let block_size =
                                if block.block_size as u64 <= MAX_BLOCK_SIZE as u64{
                                    block.block_size as u64
                                } else {
                                    MAX_BLOCK_SIZE as u64
                                };
                            let stream_id = (4 * (client.dtp_config_offset + 1) + 1) as u64;
                            let depend_id = stream_id;
                            let mut len = 0usize;
                            while len < block_size as usize {
                                unsafe {
                                    len += match client.conn.stream_send_full(stream_id as u64, &DATA_BUF[0..1_000_000], block_size as usize - len <= 1000000, deadline, priority, depend_id) {
                                        Ok(l) => l,
                                        Err(err) => panic!("{:?}", err),
                                    };
                                }
                            }
                            if (len as u64) != block_size {
                                panic!("sent length < block size: {:?}, {:?}", len, block.block_size);
                            }
                            debug!("sent block {}", client.dtp_config_offset);

                            total_bytes += len;
                            client.dtp_config_offset += 1;
                            if send_time_gap >= 0.005 {
                                // Send a block each round
                                client.next_timeout = Some(Instant::now() + Duration::from_secs_f32(send_time_gap));
                                break;
                            } else {
                                // Send the block immediately
                                continue;
                            }
                        }
                    }
                }
            }
        }

        // Generate outgoing QUIC packets for all active connections and send
        // them on the UDP socket, until quiche reports that there are no more
        // packets to be sent.
        for (peer, client) in clients.values_mut() {
            loop {
                let write = match client.conn.send(&mut out) {
                    Ok(v) => v,

                    Err(quiche::Error::Done) => {
                        debug!("{} done writing", client.conn.trace_id());
                        break;
                    },

                    Err(e) => {
                        error!("{} send failed: {:?}", client.conn.trace_id(), e);
                        client.conn.close(false, 0x1, b"fail").ok();
                        break;
                    },
                };

                // TODO: coalesce packets.
                if let Err(e) = socket.send_to(&out[..write], *peer) {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("send() would block");
                        break;
                    }

                    panic!("send() failed: {:?}", e);
                }

                debug!("{} written {} bytes", client.conn.trace_id(), write);
            }
        }

        let mut delete_num = 0;
        // Garbage collect closed connections.
        clients.retain(|_, (_, ref mut c)| {
            debug!("Collecting garbage");

            if c.conn.is_closed() {
                info!(
                    "{} connection collected {:?}",
                    c.conn.trace_id(),
                    c.conn.stats()
                );

                delete_num += 1;
                let end_timestamp = Some(get_current_usec());
                c.end_time = Some(get_current_usec());
                eprintln!("connection closed, you can see result in client.log");
                log!(file, display, "connection closed, you can see result in client.log\n");

                let total_time = end_timestamp.unwrap() - start_timestamp.unwrap();
                let _c_duration =  c.end_time.unwrap() - c.start_time.unwrap();
                eprintln!("total_bytes={}, total_time(us)={}, throughput(B/s)={}", total_bytes, total_time, total_bytes as f64 / (total_time as f64/ 1000.0 / 1000.0));
                log!(file, display, "total_bytes={}, total_time(us)={}, throughput(B/s)={}\n", total_bytes, total_time, total_bytes as f64 / (total_time as f64/ 1000.0 / 1000.0));
                eprintln!("server stat: {:?}", c.conn.stats());
                log!(file, display, "server stat: {:?}", c.conn.stats());
            }
            !c.conn.is_closed()
        });

        // Stop loop for one single client test
        if delete_num == 1 {
            break;
        }
    }

    eprintln!("Server stopped normally");
    Ok(())
}


/// Generate a stateless retry token.
///
/// The token includes the static string `"quiche"` followed by the IP address
/// of the client and by the original destination connection ID generated by the
/// client.
///
/// Note that this function is only an example and doesn't do any cryptographic
/// authenticate of the token. *It should not be used in production system*.
fn mint_token(hdr: &quiche::Header, src: &net::SocketAddr) -> Vec<u8> {
    let mut token = Vec::new();

    token.extend_from_slice(b"quiche");

    let addr = match src.ip() {
        std::net::IpAddr::V4(a) => a.octets().to_vec(),
        std::net::IpAddr::V6(a) => a.octets().to_vec(),
    };

    token.extend_from_slice(&addr);
    token.extend_from_slice(&hdr.dcid);

    token
}

/// Validates a stateless retry token.
///
/// This checks that the ticket includes the `"quiche"` static string, and that
/// the client IP address matches the address stored in the ticket.
///
/// Note that this function is only an example and doesn't do any cryptographic
/// authenticate of the token. *It should not be used in production system*.
fn validate_token<'a>(
    src: &net::SocketAddr, token: &'a [u8],
) -> Option<&'a [u8]> {
    if token.len() < 6 {
        return None;
    }

    if &token[..6] != b"quiche" {
        return None;
    }

    let token = &token[6..];

    let addr = match src.ip() {
        std::net::IpAddr::V4(a) => a.octets().to_vec(),
        std::net::IpAddr::V6(a) => a.octets().to_vec(),
    };

    if token.len() < addr.len() || &token[..addr.len()] != addr.as_slice() {
        return None;
    }

    let token = &token[addr.len()..];

    Some(&token[..])
}

fn send_config(client: &mut Client, configs: &Vec<dtp_config>) -> Result<(), quiche::Error> {
    if client.dtp_config_offset >= configs.len() {
        return Ok(());
    }

    for i in client.dtp_config_offset..configs.len() {
        let send_time_gap = configs[i].send_time_gap;
        let deadline = configs[i].deadline as u64;
        let priority = configs[i].priority as u64;
        let block_size =
            if configs[i].block_size as u64 <= MAX_BLOCK_SIZE as u64{
                configs[i].block_size as u64
            } else {
                MAX_BLOCK_SIZE as u64
            };

        let stream_id = (4 * (client.dtp_config_offset + 1) + 1) as u64;
        let depend_id = stream_id;

        unsafe {
            let len = match client.conn.stream_send_full(stream_id as u64, &DATA_BUF[0..block_size as usize], true, deadline, priority, depend_id) {
                Ok(l) => l,
                Err(quiche::Error::Done) => 0, // Would block
                Err(err ) => return Err(err),
            };
        }


    }

    Ok(())
}

/// Handles incoming HTTP/0.9 requests.
fn handle_stream(_client: &mut Client, _stream_id: u64, _buf: &[u8], _root: &str) {
    ()
}

/// Handles newly writable streams.
fn handle_writable(client: &mut Client, stream_id: u64) {
    let conn = &mut client.conn;

    debug!("{} stream {} is writable", conn.trace_id(), stream_id);

    // if stream_id has been sent

}

fn hex_dump(buf: &[u8]) -> String {
    let vec: Vec<String> = buf.iter().map(|b| format!("{:02x}", b)).collect();

    vec.join("")
}
