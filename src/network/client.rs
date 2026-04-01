use crate::network::bundle::ProtobufBundle;
use crate::network::protobuf::{deserialize, serialize};
use crate::network::server::{ServerRequest, ServerResponse};
use crate::routing::model::{Bundle, BundleKind, Node};
use crate::routing::RoutingEngine;
use serde_json;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use tokio::runtime::Builder;
use uuid::Uuid;

pub fn connect_to_server(node: Node) -> Option<TcpStream> {
    let address = "127.0.0.1:8080";
    match connect_with_retry(&address, 3, 2) {
        Some(mut stream) => {
            let message = serde_json::to_string(&ServerRequest::Register(node)).unwrap_or_default();
            if stream.write_all(message.as_bytes()).is_err() {
                return None;
            }
            // ✅ Set a timeout so read doesn't block forever
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .ok();

            let mut buf = [0u8; 1024];
            match stream.read(&mut buf) {
                Ok(_) => {}
                Err(_) => {}
                // Either way we continue — ack is best-effort
            }

            // ✅ Remove the timeout so the persistent connection doesn't time out later
            stream.set_read_timeout(None).ok();

            Some(stream)
        }
        None => None,
    }
}

pub fn get_connected_peers_from_server(
    requested_ids: &[Uuid],
) -> Vec<crate::network::server::PeerRecord> {
    let address = "127.0.0.1:8080";
    let mut stream = match connect_with_retry(address, 3, 2) {
        Some(s) => s,
        None => {
            eprintln!("get_connected_peers_from_server: could not reach server");
            return vec![];
        }
    };

    let msg = serde_json::to_vec(&crate::network::server::ServerRequest::GetConnectedPeers(
        requested_ids.to_vec(),
    ))
    .unwrap_or_default();

    if stream.write_all(&msg).is_err() {
        return vec![];
    }

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .ok();
    let mut buf = [0u8; 16384];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return vec![],
    };

    match serde_json::from_slice::<crate::network::server::ServerResponse>(&buf[..n]) {
        Ok(crate::network::server::ServerResponse::Peers(peers)) => peers,
        _ => vec![],
    }
}

//Connection Retry & Failure Handling
// A helper function that attempts to establish a TCP connection multiple times
// with a delay between attempts. This prevents the node from giving up immediately
// if the target peer is temporarily offline or experiencing high latency.
fn connect_with_retry(address: &str, max_retries: u32, delay_secs: u64) -> Option<TcpStream> {
    for attempt in 1..=max_retries {
        match TcpStream::connect(address) {
            Ok(stream) => {
                // If successful, return the open connection immediately
                return Some(stream);
            }
            Err(e) => {
                eprintln!(
                    "Network Warning: Connection to {} failed (Attempt {}/{}): {}",
                    address, attempt, max_retries, e
                );

                // If we haven't reached the max retries, wait and try again
                if attempt < max_retries {
                    //pause the current thread for the specified delay before the next attempt
                    //prevents the node from spamming a struggling server with thousands of requests per second
                    thread::sleep(Duration::from_secs(delay_secs));
                }
            }
        }
    }

    //If all attempts fail, we log it and return None instead of crashing
    eprintln!(
        "Network Error: Exhausted all {} attempts to connect to {}. Node is unreachable.",
        max_retries, address
    );
    None
}

fn connect_to_peer(source_id: Uuid, destination: String) -> Option<TcpStream> {
    match connect_with_retry(&destination, 3, 2) {
        Some(stream) => Some(stream),
        None => {
            eprintln!(
                "{} Failed to connect to {} after all retries.",
                source_id, destination
            );
            None
        }
    }
}

pub fn send_bundle(source_id: Uuid, bundle: &Bundle, destination: String) {
    let proto_bundle = ProtobufBundle::from(bundle);
    let payload = match serialize(&proto_bundle) {
        Some(bytes) => bytes,
        None => {
            eprintln!("send_bundle: failed to serialize bundle");
            return;
        }
    };

    // Connect to the destination first
    let mut stream = match connect_to_peer(source_id, destination) {
        Some(s) => s,
        None => {
            eprintln!("send_bundle: could not connect to destination");
            return;
        }
    };

    // send length prefix then payload
    let len = payload.len() as u32;
    if let Err(e) = stream
        .write_all(&len.to_be_bytes())
        .and_then(|_| stream.write_all(&payload))
    {
        eprintln!("send_bundle failed to write to : {}", e);
    }
    let mut ack = [0u8; 4];
    if let Err(e) = stream.read_exact(&mut ack) {
        eprintln!("send_bundle: failed to read ack from  {}", e);
    }
}

pub fn receive_bundle(stream: &mut TcpStream) -> Option<Bundle> {
    // read the length prefix (4 bytes)
    let mut len_buf = [0u8; 4];
    if let Err(e) = stream.read_exact(&mut len_buf) {
        eprintln!("receive_bundle: failed to read length prefix: {}", e);
        return None;
    }

    let len = u32::from_be_bytes(len_buf) as usize;

    // read exactly `len` bytes
    let mut payload = vec![0u8; len];
    if let Err(e) = stream.read_exact(&mut payload) {
        eprintln!("receive_bundle: failed to read payload: {}", e);
        return None;
    }

    // deserialize the protobuf bytes into a Bundle
    let bundle = match deserialize(&payload) {
        Some(proto_bundle) => Bundle::from(proto_bundle),
        None => {
            eprintln!("receive_bundle: failed to deserialize bundle");
            return None;
        }
    };

    // send ack back
    if let Err(e) = stream.write_all(b"ack\n") {
        eprintln!("receive_bundle: failed to send ack: {}", e);
    }

    Some(bundle)
}

pub fn request_peer_sv(
    self_id: Uuid,
    destination: String,
) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
    let mut stream = match connect_with_retry(&destination, 3, 2) {
        Some(s) => s,
        None => return Err(format!("could not reach peer {}", destination).into()),
    };

    let msg = BundleKind::RequestSV { from: self_id };
    let payload = serde_json::to_vec(&msg)?;
    stream.write_all(&payload)?;

    // ✅ read length prefix first, then payload
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buffer = vec![0u8; len];
    stream.read_exact(&mut buffer)?;

    match serde_json::from_slice::<BundleKind>(&buffer)? {
        BundleKind::SummaryVector { ids } => Ok(ids),
        _ => Err("unexpected response from peer".into()),
    }
}

pub fn respond_peer_sv(stream: &mut TcpStream, routing_engine: &mut RoutingEngine) {
    let mut buffer = [0u8; 4096];

    let n = match stream.read(&mut buffer) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("respond_peer_sv: failed to read request: {}", e);
            return;
        }
    };

    match serde_json::from_slice::<BundleKind>(&buffer[..n]) {
        Ok(BundleKind::RequestSV { from: _ }) => {
            let ids: Vec<Uuid> = routing_engine
                .bundle_manager
                .get_bundles_from_node()
                .iter()
                .map(|b| b.id)
                .collect();

            let response = BundleKind::SummaryVector { ids };
            let payload = match serde_json::to_vec(&response) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("respond_peer_sv: failed to serialize response: {}", e);
                    return;
                }
            };

            // Send length prefix then payload
            let len = payload.len() as u32;
            if let Err(e) = stream.write_all(&len.to_be_bytes()) {
                eprintln!("respond_peer_sv: failed to send length prefix: {}", e);
                return;
            }

            if let Err(e) = stream.write_all(&payload) {
                eprintln!("respond_peer_sv: failed to send SummaryVector: {}", e);
            }
        }
        _ => {
            eprintln!("respond_peer_sv: unexpected message, expected RequestSV");
        }
    }
}

pub fn handle_peer_to_peer(mut stream: TcpStream, routing_engine: &mut RoutingEngine) {
    loop {
        // Inspect the first byte without consuming it to choose the right handler.
        // JSON control messages start with '{', while bundle transfers use a 4-byte length prefix.
        let mut first = [0u8; 1];
        let n = match stream.peek(&mut first) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("handle_peer_to_peer: failed to peek stream: {}", e);
                break;
            }
        };

        if n == 0 {
            break;
        }

        if first[0] == b'{' {
            respond_peer_sv(&mut stream, routing_engine);
            continue;
        }

        match receive_bundle(&mut stream) {
            Some(mut bundle) => {
                if routing_engine.bundle_manager.has_bundle(bundle.id) {
                    continue;
                }

                let runtime = match Builder::new_current_thread().enable_all().build() {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        eprintln!("handle_peer_to_peer: failed to create runtime: {}", e);
                        break;
                    }
                };

                runtime.block_on(routing_engine.route_bundle(&mut bundle));
            }
            None => {
                eprintln!("handle_peer_to_peer: failed to receive bundle");
                break;
            }
        }
    }
}
