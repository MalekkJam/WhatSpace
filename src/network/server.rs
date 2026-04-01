use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::routing::model::Node;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub node: Node,
    pub status: ConnectionStatus,
}

// Server memory of registered peers (keyed by node id).
pub type PeerRegistry = Arc<Mutex<Vec<PeerRecord>>>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerRequest {
    Register(Node),
    GetConnectedPeers(Vec<Uuid>), // List of node IDs to query for
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerResponse {
    Ok,
    Peers(Vec<PeerRecord>), // List of connected peer records
    Error(String),
}
#[derive(Debug, Clone)]
pub struct Server {
    pub peer_registry: PeerRegistry,
}

impl Server {
    pub fn new() -> Self {
        Server {
            peer_registry: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start_server(&self) {
        let listener = TcpListener::bind("127.0.0.1:9100").expect("Failed to bind to address"); // this binds registry on a port that avoids common local conflicts on 8080
        println!("Server listening on 127.0.0.1:9100"); // this prints the effective registry endpoint for startup verification

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let registry_clone = Arc::clone(&self.peer_registry);
                    std::thread::spawn(move || {
                        let server = Server {
                            peer_registry: registry_clone,
                        };
                        server.handle_client(stream);
                    });
                }
                Err(e) => eprintln!("Failed to establish connection: {}", e),
            }
        }
    }

    pub fn disconnect_server(&self) {
        // Mark all nodes as disconnected
        match self.peer_registry.lock() {
            Ok(mut map) => {
                for record in map.iter_mut() {
                    record.status = ConnectionStatus::Disconnected; // change the status to disconnected
                    println!("Node {} marked as disconnected", record.node.id);
                }
                println!("Server disconnected all nodes marked as disconnected");
            }
            Err(e) => {
                eprintln!("Failed to acquire lock on registry: {}", e);
            }
        }

        // Gracefully shutdown the server process (equivalent to Ctrl+C)
        println!("Shutting down server");
        std::process::exit(0); // ctrl+c
    }

    pub fn get_connected_peers(&self, requested_ids: &[Uuid]) -> Vec<PeerRecord> {
        match self.peer_registry.lock() {
            Ok(map) => map
                .iter()
                .filter(|record| {
                    record.status == ConnectionStatus::Connected
                        && (requested_ids.is_empty() || requested_ids.contains(&record.node.id)) // return only requested connected peers, or all connected peers when caller provides no ids
                })
                .cloned()
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn handle_client(&self, mut stream: TcpStream) {
        let mut node_id: Option<Uuid> = None;
        let mut buffer = [0_u8; 4096];
        eprintln!("we are in the handle client");
        loop {
            let n = match stream.read(&mut buffer) {
                Ok(0) => {
                    println!("DEBUG: got EOF (client closed connection)");
                    break;
                } // Connection closed by client
                Ok(n) => {
                    println!("DEBUG: read {} bytes", n);
                    n
                }
                Err(e) => {
                    eprintln!("Failed to read from client: {}", e);
                    break;
                }
            };

            let request: ServerRequest = match serde_json::from_slice(&buffer[..n]) {
                Ok(req) => req,
                Err(e) => {
                    let _ = write_json(
                        &mut stream,
                        &ServerResponse::Error(format!("Invalid request JSON: {}", e)),
                    );
                    continue;
                }
            };
            match request {
                ServerRequest::Register(node) => {
                    node_id = Some(node.id);
                    if let Ok(mut map) = self.peer_registry.lock() {
                        if let Some(existing) = map.iter_mut().find(|record| record.node.id == node.id) {
                            existing.node = node; // refresh node metadata so address and peer list stay up to date on reconnect
                            existing.status = ConnectionStatus::Connected; // mark existing record as alive instead of creating duplicates
                        } else {
                            map.push(PeerRecord {
                                node,
                                status: ConnectionStatus::Connected,
                            }); // insert new node into registry the first time we see this id
                        }
                        let _ = write_json(&mut stream, &ServerResponse::Ok);
                    } else {
                        let _ = write_json(
                            &mut stream,
                            &ServerResponse::Error("Registry lock poisoned".to_string()),
                        );
                    }
                }
                ServerRequest::GetConnectedPeers(requested_ids) => {
                    let peers = self.get_connected_peers(&requested_ids);
                    let _ = write_json(&mut stream, &ServerResponse::Peers(peers));
                }
            }
        }

        // Connection closed - mark node as disconnected
        if let Some(id) = node_id {
            if let Ok(mut map) = self.peer_registry.lock() {
                if let Some(record) = map.iter_mut().find(|r| r.node.id == id) {
                    record.status = ConnectionStatus::Disconnected;
                    println!("Node {} marked as disconnected", id);
                }
            }
        }
    }
}

fn write_json(stream: &mut TcpStream, response: &ServerResponse) -> std::io::Result<()> {
    let body = serde_json::to_vec(response)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    stream.write_all(&body)
}

pub fn verify_unique_name(registry: &PeerRegistry, name: &str) -> bool {
    match registry.lock() {
        Ok(map) => !map.iter().any(|record| record.node.name == name),
        Err(_) => false,
    }
}
