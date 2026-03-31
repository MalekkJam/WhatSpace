use crate::network::server::{
    get_connected_peers, PeerRecord, PeerRegistry, ServerRequest, ServerResponse,
};
use crate::routing::model::{Bundle, Node};
use crate::network::protobuf::{serialize,deserialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;

pub fn connect_to_server(node: &Node) -> bool {
    let address = format!("{}:{}", node.address, node.port);
    match TcpStream::connect(&address) {
        Ok(mut stream) => {
            println!("Connecté à {}", address);

            let message =
                serde_json::to_string(&ServerRequest::Register(node.clone())).unwrap_or_default();

            match stream.write_all(message.as_bytes()) {
                Ok(_) => {
                    println!("Données envoyées au serveur !");
                    true
                }
                Err(e) => {
                    println!("Erreur d'envoi: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            println!("Erreur de connexion: {}", e);
            false
        }
    }
}

pub fn connect_to_peer(source: &Node, destination: &Node) -> Option<TcpStream> {
    let addr = format!("{}:{}", destination.address, destination.port);
    match TcpStream::connect(&addr) {
        Ok(stream) => {
            println!("[{}] Connected to peer at {}", source.id, addr);
            Some(stream)
        }
        Err(e) => {
            eprintln!("[{}] Failed to connect to {}: {}", source.id, addr, e);
            None
        }
    }
}

pub fn connect_to_peers(node: &Node) -> Vec<(PeerRecord, TcpStream)> {
    let connected_peers = request_connected_peers(node, node.peers.clone());

    if connected_peers.is_empty() {
        return Vec::new();
    }

    connected_peers
        .into_iter()
        .filter_map(|record| connect_to_peer(node, &record.node).map(|stream| (record, stream)))
        .collect()
}

// Call the get_connected_peers on the server
// Executed the ServerRequest::GetConnectedPeers
fn request_connected_peers(node: &Node, requested_ids: Vec<Uuid>) -> Vec<PeerRecord> {
    let address = format!("{}:{}", node.address, node.port);

    let mut stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("request_connected_peers failed to connect to server: {}", e);
            return Vec::new();
        }
    };

    // send the request
    let message =
        serde_json::to_string(&ServerRequest::GetConnectedPeers(requested_ids)).unwrap_or_default();

    if let Err(e) = stream.write_all(message.as_bytes()) {
        eprintln!("request_connected_peers failed to send request: {}", e);
        return Vec::new();
    }

    // read the response
    let mut buffer = [0u8; 4096];
    match stream.read(&mut buffer) {
        Ok(n) => match serde_json::from_slice::<ServerResponse>(&buffer[..n]) {
            Ok(ServerResponse::Peers(peers)) => peers,
            Ok(ServerResponse::Error(e)) => {
                eprintln!("request_connected_peers server error: {}", e);
                Vec::new()
            }
            _ => Vec::new(),
        },
        Err(e) => {
            eprintln!("request_connected_peers failed to read response: {}", e);
            Vec::new()
        }
    }
}

pub fn send_bundle(source: &Node, bundle: Bundle) {
    let peers = connect_to_peers(source);

    if peers.is_empty() {
        eprintln!("send_bundle: no connected peers");
        return;
    }

    let proto_bundle = bundle.into();
    let payload = match serialize(&proto_bundle) {
        Some(bytes) => bytes,
        None => {
            eprintln!("send_bundle: failed to serialize bundle");
            return;
        }
    };

    for (record, mut stream) in peers {
        let address = format!("{}:{}", record.node.address, record.node.port);
        // send length prefix then payload
        let len = payload.len() as u32;
        if let Err(e) = stream
            .write_all(&len.to_be_bytes())
            .and_then(|_| stream.write_all(&payload))
        {
            eprintln!("send_bundle failed to write to {}: {}", address, e);
            continue; // skip this peer, try the next
        }
        let mut ack = [0u8; 4];
        if let Err(e) = stream.read_exact(&mut ack) {
            eprintln!("send_bundle: failed to read ack from {}: {}", address, e);
        }
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