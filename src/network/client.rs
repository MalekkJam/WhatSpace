use crate::network::bundle::ProtobufBundle; // this import gives access to the protobuf wire model for bundles
use crate::network::protobuf::{deserialize, serialize}; // this import provides conversion helpers between bytes and protobuf bundles
use crate::network::server::{ServerRequest, ServerResponse}; // this import provides registry request/response models used by registration and peer lookup
use crate::routing::model::{Bundle, BundleKind, Node}; // this import exposes runtime bundle and node types used by routing and cli
use crate::routing::RoutingEngine; // this import exposes the routing engine that processes incoming bundles
use std::io::{Read, Write}; // this import provides blocking read and write operations for tcp streams
use std::net::TcpStream; // this import provides tcp client primitives
use std::thread; // this import provides thread spawning for blocking listeners
use std::time::Duration; // this import provides sleep and timeout durations used in retries
use uuid::Uuid; // this import provides stable identifiers for nodes and bundles

const DEFAULT_REGISTRY_ADDR: &str = "127.0.0.1:9100"; // this constant defines the fallback registry address when no server is provided
const ACK_BYTES: [u8; 4] = *b"ACK!"; // this constant defines a fixed-size ack payload for bundle transfer confirmation

pub fn connect_to_server(node: Node, server_addr: Option<&str>) -> Option<TcpStream> {
    let address = server_addr.unwrap_or(DEFAULT_REGISTRY_ADDR); // this selects the user-provided registry endpoint or the default one
    match connect_with_retry(address, 3, 1) {
        Some(mut stream) => {
            let message = match serde_json::to_vec(&ServerRequest::Register(node)) {
                Ok(bytes) => bytes, // this path serializes registration payload successfully
                Err(e) => {
                    eprintln!("register serialization failed: {}", e); // this logs serialization failure so start command can be diagnosed
                    return None; // this aborts startup because registration payload is invalid
                }
            };

            if stream.write_all(&message).is_err() {
                eprintln!("failed to send register request"); // this logs transport write failure during registration
                return None; // this aborts startup because registry did not receive registration data
            }

            stream.set_read_timeout(Some(Duration::from_secs(2))).ok(); // this prevents permanent blocking while waiting for server acknowledgement
            let mut buf = [0u8; 4096]; // this buffer stores one registry response frame
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    let _ = serde_json::from_slice::<ServerResponse>(&buf[..n]); // this attempts to parse ack to validate payload shape without failing startup on parse errors
                }
            }
            stream.set_read_timeout(None).ok(); // this restores blocking mode so the persistent connection can remain open

            Some(stream) // this returns the persistent stream so node presence remains registered until stream drops
        }
        None => None, // this propagates connection failure when retry budget is exhausted
    }
}

fn connect_with_retry(address: &str, max_retries: u32, delay_secs: u64) -> Option<TcpStream> {
    for attempt in 1..=max_retries {
        match TcpStream::connect(address) {
            Ok(stream) => {
                println!(
                    "Network: Successfully connected to {} (Attempt {}/{})",
                    address, attempt, max_retries
                ); // this logs successful connection establishment with retry metadata
                return Some(stream); // this exits early on the first successful connection
            }
            Err(e) => {
                eprintln!(
                    "Network Warning: Connection to {} failed (Attempt {}/{}): {}",
                    address, attempt, max_retries, e
                ); // this logs the failure reason for each retry attempt

                if attempt < max_retries {
                    thread::sleep(Duration::from_secs(delay_secs)); // this avoids busy-looping retries and gives remote peers time to recover
                }
            }
        }
    }

    eprintln!(
        "Network Error: Exhausted all {} attempts to connect to {}.",
        max_retries, address
    ); // this logs final failure after all retries are consumed
    None // this returns failure to caller so routing can fallback to store-carry-forward behavior
}

fn connect_to_peer(source_id: Uuid, destination: &str) -> Option<TcpStream> {
    match connect_with_retry(destination, 3, 1) {
        Some(stream) => {
            println!("{} connected to peer at {}", source_id, destination); // this logs successful peer dial for observability during forwarding
            Some(stream) // this returns the active peer stream
        }
        None => {
            eprintln!("{} failed to connect to {}", source_id, destination); // this logs connection failure so anti-entropy retries can be understood
            None // this propagates failure so caller can keep bundle pending
        }
    }
}

pub fn send_bundle(source_id: Uuid, bundle: &Bundle, destination: String) {
    let proto_bundle = ProtobufBundle::from(bundle); // this converts runtime bundle into protobuf transport representation
    let payload = match serialize(&proto_bundle) {
        Some(bytes) => bytes, // this path uses serialized protobuf payload for transport
        None => {
            eprintln!("send_bundle: failed to serialize bundle"); // this logs serialization failure before network send attempt
            return; // this aborts send when payload is invalid
        }
    };

    let mut stream = match connect_to_peer(source_id, &destination) {
        Some(stream) => stream, // this opens a tcp connection to destination node listener
        None => return, // this exits while keeping bundle pending for future anti-entropy retries
    };

    let len = payload.len() as u32; // this encodes payload size so the receiver can read one exact bundle frame
    if let Err(e) = stream
        .write_all(&len.to_be_bytes())
        .and_then(|_| stream.write_all(&payload))
    {
        eprintln!("send_bundle failed to write payload: {}", e); // this logs framing/transport write errors during bundle transfer
        return; // this aborts send when payload cannot be transmitted
    }

    let mut ack = [0u8; 4]; // this buffer receives fixed-size peer acknowledgement
    if let Err(e) = stream.read_exact(&mut ack) {
        eprintln!("send_bundle: failed to read ack: {}", e); // this logs missing or broken acknowledgements from peers
    }
}

pub fn receive_bundle(stream: &mut TcpStream) -> Option<Bundle> {
    let mut len_buf = [0u8; 4]; // this buffer reads the fixed-size bundle frame length prefix
    if let Err(e) = stream.read_exact(&mut len_buf) {
        eprintln!("receive_bundle: failed to read frame length: {}", e); // this logs frame header read failures
        return None; // this stops processing when payload boundaries cannot be decoded
    }

    let len = u32::from_be_bytes(len_buf) as usize; // this decodes payload size from network-byte-order length header
    let mut payload = vec![0u8; len]; // this allocates exact buffer size for one inbound bundle payload
    if let Err(e) = stream.read_exact(&mut payload) {
        eprintln!("receive_bundle: failed to read frame payload: {}", e); // this logs payload read failures for synchronization debugging
        return None; // this stops processing when full payload is unavailable
    }

    let bundle = match deserialize(&payload) {
        Some(proto_bundle) => Bundle::from(proto_bundle), // this converts protobuf wire payload into runtime bundle model
        None => {
            eprintln!("receive_bundle: failed to deserialize bundle"); // this logs decode errors for corrupted/invalid payloads
            return None; // this signals invalid payload to caller
        }
    };

    if let Err(e) = stream.write_all(&ACK_BYTES) {
        eprintln!("receive_bundle: failed to send ack: {}", e); // this logs acknowledgement write failure to sender
    }

    Some(bundle) // this returns decoded bundle to the routing handler
}

pub fn request_peer_sv(self_id: Uuid, destination: String) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
    let mut stream = connect_with_retry(&destination, 3, 1)
        .ok_or_else(|| format!("could not reach peer {}", destination))?; // this opens peer control connection for summary-vector exchange

    let request = BundleKind::RequestSV { from: self_id }; // this builds a summary-vector request message carrying local node id
    let payload = serde_json::to_vec(&request)?; // this serializes request message to framed json bytes
    let len = payload.len() as u32; // this encodes request payload size so peer reads exactly one control frame
    stream.write_all(&len.to_be_bytes())?; // this writes control frame length for request synchronization
    stream.write_all(&payload)?; // this writes request payload bytes

    let mut response_len = [0u8; 4]; // this reads the peer response frame length header
    stream.read_exact(&mut response_len)?; // this blocks until full response header arrives
    let response_size = u32::from_be_bytes(response_len) as usize; // this decodes expected response payload size
    let mut response_payload = vec![0u8; response_size]; // this allocates buffer for full summary-vector response
    stream.read_exact(&mut response_payload)?; // this reads one complete summary-vector payload
    match serde_json::from_slice::<BundleKind>(&response_payload)? {
        BundleKind::SummaryVector { ids } => Ok(ids), // this returns the peer inventory ids needed by anti-entropy
        _ => Err("unexpected response from peer".into()), // this fails when peer replies with wrong control message type
    }
}

pub fn respond_peer_sv(stream: &mut TcpStream, routing_engine: &RoutingEngine, request: BundleKind) {
    match request {
        BundleKind::RequestSV { from } => {
            println!("[{}] received RequestSV from {}", routing_engine.node_id, from); // this logs inbound summary-vector queries for traceability

            let ids: Vec<Uuid> = routing_engine
                .get_summary_vector(&routing_engine.bundle_manager)
                .iter()
                .map(|bundle| bundle.id)
                .collect(); // this computes local summary-vector by extracting bundle ids from storage

            let response = BundleKind::SummaryVector { ids }; // this builds summary-vector response control message
            let payload = match serde_json::to_vec(&response) {
                Ok(payload) => payload, // this serializes control response to bytes for framed transport
                Err(e) => {
                    eprintln!("respond_peer_sv: failed to serialize response: {}", e); // this logs serialization failures before socket write
                    return; // this exits handler when response cannot be encoded
                }
            };

            let len = payload.len() as u32; // this encodes response size so requester can read one exact control frame
            if let Err(e) = stream
                .write_all(&len.to_be_bytes())
                .and_then(|_| stream.write_all(&payload))
            {
                eprintln!("respond_peer_sv: failed to send SummaryVector: {}", e); // this logs socket write failures for summary-vector response
            }
        }
        _ => {
            eprintln!("respond_peer_sv: unexpected control message"); // this logs protocol misuse if handler receives unsupported control message
        }
    }
}

pub fn handle_peer_to_peer(mut stream: TcpStream, routing_engine: &mut RoutingEngine) {
    loop {
        let mut len_buf = [0u8; 4]; // this buffer reads one incoming frame length header for either control or bundle traffic
        if stream.read_exact(&mut len_buf).is_err() {
            break; // this stops loop cleanly when peer disconnects or frame header cannot be read
        }
        let len = u32::from_be_bytes(len_buf) as usize; // this decodes frame payload size to preserve stream synchronization
        let mut payload = vec![0u8; len]; // this allocates exact frame payload buffer
        if stream.read_exact(&mut payload).is_err() {
            break; // this stops loop when payload is incomplete or stream closes mid-frame
        }

        if let Ok(control) = serde_json::from_slice::<BundleKind>(&payload) {
            respond_peer_sv(&mut stream, routing_engine, control); // this serves anti-entropy control-plane requests on the same tcp channel
            continue; // this loops to handle next framed message from the same peer
        }

        let mut bundle = match deserialize(&payload) {
            Some(proto_bundle) => Bundle::from(proto_bundle), // this decodes payload as protobuf bundle when it is not a control json frame
            None => {
                eprintln!("handle_peer_to_peer: unknown payload"); // this logs protocol violations for malformed messages
                continue; // this skips malformed frames without killing the whole listener
            }
        };

        if let Err(e) = stream.write_all(&ACK_BYTES) {
            eprintln!("handle_peer_to_peer: failed to send ack: {}", e); // this logs ack failures so sender-side retries can be investigated
            break; // this closes current connection if sender can no longer receive acknowledgements
        }

        let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime, // this builds a local runtime so we can execute async routing from this blocking tcp thread
            Err(e) => {
                eprintln!("handle_peer_to_peer: failed to build runtime: {}", e); // this logs runtime creation failure for debugging
                break; // this closes the connection when routing cannot be executed safely
            }
        };
        runtime.block_on(routing_engine.route_bundle(&mut bundle)); // this executes route logic immediately after transport ack to keep layers synchronized
    }
}
