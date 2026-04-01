use crate::network::bundle::ProtobufBundle;
use crate::network::protobuf::{deserialize, serialize};
use crate::network::server::ServerRequest;
use crate::routing::model::{Bundle, BundleKind, Node};
use serde_json;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

pub fn connect_to_server(node: Node) -> Option<TcpStream> {
    let address = "127.0.0.1:8080";
    match connect_with_retry(&address, 3, 2) {
        Some(mut stream) => {
            let message = serde_json::to_string(&ServerRequest::Register(node)).unwrap_or_default();
            println!("DEBUG client: sending register message");
            if stream.write_all(message.as_bytes()).is_err() {
                return None;
            }
            // ✅ Set a timeout so read doesn't block forever
            stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok();

            let mut buf = [0u8; 1024];
            match stream.read(&mut buf) {
                Ok(n) => println!("DEBUG client: got ack ({} bytes)", n),
                Err(e) => println!("DEBUG client: ack read timed out or failed: {}", e),
                // Either way we continue — ack is best-effort
            }

            // ✅ Remove the timeout so the persistent connection doesn't time out later
            stream.set_read_timeout(None).ok();

            println!("DEBUG client: returning stream");
            Some(stream)
        }
        None => None,
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
                println!(
                    "Network: Successfully connected to {} (Attempt {}/{})",
                    address, attempt, max_retries
                );
                return Some(stream);
            }
            Err(e) => {
                eprintln!(
                    "Network Warning: Connection to {} failed (Attempt {}/{}): {}",
                    address, attempt, max_retries, e
                );

                // If we haven't reached the max retries, wait and try again
                if attempt < max_retries {
                    println!("Network: Retrying in {} seconds...", delay_secs);
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
        Some(stream) => {
            println!("{} Connected to peer at {}", source_id, destination);
            Some(stream)
        }
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
    // askiing another peer a qestion " what bundles do u have "
    let mut stream = match connect_with_retry(&destination, 3, 2) {
        Some(s) => s,
        None => return Err(format!("could not reach peer {}", destination).into()),
    };

    let msg = BundleKind::RequestSV { from: self_id };
    let payload = serde_json::to_vec(&msg)?;
    stream.write_all(&payload)?;

    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer)?;

    match serde_json::from_slice::<BundleKind>(&buffer[..n])? {
        BundleKind::SummaryVector { ids } => Ok(ids),
        _ => Err("unexpected response from peer".into()),
    }
}
