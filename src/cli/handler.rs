use crate::cli::cli::{NodeCommands, PeerCommands};
use crate::network::client::{connect_to_server, handle_peer_to_peer};
use crate::routing::model::{Bundle, BundleKind, Node};
use crate::routing::RoutingEngine;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

static REGISTRY_STREAMS: LazyLock<Mutex<HashMap<Uuid, TcpStream>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static LISTENING_PORTS: LazyLock<Mutex<HashSet<u16>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
static RETRYING_NODES: LazyLock<Mutex<HashSet<Uuid>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

fn sync_node_engine_peers(node: &mut Node) {
    if let Some(engine) = &mut node.routing_engine {
        engine.peers = node.peers.clone();
    }
}

fn find_node<'a>(nodes: &'a [Node], name: &str) -> &'a Node {
    nodes.iter().find(|n| n.name == name).unwrap_or_else(|| {
        eprintln!("No node named '{}' found. Available nodes:", name);
        for n in nodes {
            eprintln!("  - {}", n.name);
        }
        std::process::exit(1);
    })
}

fn find_node_mut<'a>(nodes: &'a mut [Node], name: &str) -> &'a mut Node {
    if let Some(pos) = nodes.iter().position(|n| n.name == name) {
        return &mut nodes[pos];
    }

    eprintln!("No node named '{}' found. Available nodes:", name);
    for n in nodes.iter() {
        eprintln!("  - {}", n.name);
    }
    std::process::exit(1);
}

pub async fn handle_command(command: NodeCommands, nodes: &mut Vec<Node>) {
    match command {
        NodeCommands::All => {
            if nodes.is_empty() {
                println!("No nodes found.");
            } else {
                println!("Nodes in demo ({}):", nodes.len());
                for node in nodes.iter() {
                    println!(
                        "  - {} | {} | {}:{} | peers: {}",
                        node.name,
                        node.id,
                        node.address,
                        node.port,
                        node.peers.len()
                    );
                }
            }
        }

        NodeCommands::Start { name, server } => {
            let node = find_node_mut(nodes, &name);

            let stream = connect_to_server(node.clone());
            if stream.is_none() {
                eprintln!("Failed to connect node {} to server", node.name);
                return;
            }
            if let Some(stream) = stream {
                REGISTRY_STREAMS
                    .lock()
                    .unwrap()
                    .insert(node.id, stream);
            }

            // ✅ Initialize routing engine on the node if not already set
            if node.routing_engine.is_none() {
                node.routing_engine = Some(RoutingEngine::new(
                    node.id,
                    node.peers.clone(),
                    node.name.clone(),
                ));
            }
            sync_node_engine_peers(node);

            let port = node.port;
            if let Ok(mut ports) = LISTENING_PORTS.lock() {
                if !ports.contains(&port) {
                    ports.insert(port);
                    let node_name = node.name.clone();
                    // ✅ Clone the engine directly from the node
                    let engine = node.routing_engine.clone().unwrap();
                    std::thread::spawn(move || {
                        let addr = format!("127.0.0.1:{}", port);
                        let listener = match TcpListener::bind(&addr) {
                            Ok(l) => {
                                println!("Node {} listening for peers on {}", node_name, addr);
                                l
                            }
                            Err(e) => {
                                eprintln!("Failed to bind {}: {}", addr, e);
                                return;
                            }
                        };
                        for incoming in listener.incoming() {
                            match incoming {
                                Ok(stream) => {
                                    let mut engine_clone = engine.clone();
                                    std::thread::spawn(move || {
                                        crate::network::client::handle_peer_to_peer(
                                            stream,
                                            &mut engine_clone,
                                        );
                                    });
                                }
                                Err(e) => eprintln!("Peer listener error: {}", e),
                            }
                        }
                    });
                }
            }

            if let Ok(mut retrying) = RETRYING_NODES.lock() {
                if retrying.insert(node.id) {
                    let node_id = node.id;
                    let mut retry_engine = node.routing_engine.clone().unwrap();
                    std::thread::spawn(move || loop {
                        std::thread::sleep(Duration::from_secs(2));

                        let is_connected = REGISTRY_STREAMS
                            .lock()
                            .map(|streams| streams.contains_key(&node_id))
                            .unwrap_or(false);

                        if !is_connected {
                            continue;
                        }

                        retry_engine.retry_pending_bundles();
                    });
                }
            }

            println!("Node {} registered with server {}", node.name, server);
        }
        NodeCommands::Stop { name } => {
            let node = find_node(nodes, &name);

            println!("Stopping node {}...", node.name);

            if REGISTRY_STREAMS.lock().unwrap().remove(&node.id).is_some() {
                println!("Node {} disconnected from server.", node.name);
            } else {
                eprintln!("Node {} was not connected to the server.", node.name);
            }
        }

        NodeCommands::Status { name } => {
            let node = find_node_mut(nodes, &name);
            let stored = node
                .routing_engine
                .as_mut()
                .map(|engine| engine.bundle_manager.all().len())
                .unwrap_or(0);

            println!("ID : {}", node.id);
            println!("Name : {}", node.name);
            println!("Address : {}:{}", node.address, node.port);
            println!("Peers : {}", node.peers.len());
            println!("Bundles : {}", stored);
        }

        NodeCommands::Send {
            from,
            to,
            message,
            ttl,
        } => {
            // look up destination first before borrowing sender as mutable
            let destination = find_node(&nodes, &to).clone();
            let sender = find_node_mut(nodes, &from);
            sync_node_engine_peers(sender);

            let mut bundle = Bundle::new(
                sender.clone(),
                destination,
                BundleKind::Data { msg: message },
                ttl,
            );

            if let Some(engine) = &mut sender.routing_engine {
                engine.route_bundle(&mut bundle).await;
            } else {
                eprintln!("No routing engine available for {}.", sender.name);
            }
        }

        NodeCommands::Peers { name, command } => {
            let known_nodes: HashMap<String, Uuid> =
                nodes.iter().map(|n| (n.name.clone(), n.id)).collect();
            let node = find_node_mut(nodes, &name);

            handle_peer_command(command, node, &known_nodes);
        }

        #[cfg(feature = "debug")]
        NodeCommands::Debug { name } => match name {
            Some(name) => {
                let node = find_node(nodes, &name);
                println!("{}", serde_json::to_string_pretty(node).unwrap());
            }
            None => {
                println!("{}", serde_json::to_string_pretty(nodes).unwrap());
            }
        },
    }
}

fn handle_peer_command(
    command: PeerCommands,
    node: &mut Node,
    known_nodes: &HashMap<String, Uuid>,
) {
    match command {
        PeerCommands::ListPeers => {
            if node.peers.is_empty() {
                println!("No known peers for {}.", node.name);
            } else {
                println!("Peers for {}:", node.name);
                for peer in &node.peers {
                    println!("  - {}", peer);
                }
            }
        }

        PeerCommands::GetConnectedPeers { ids } => {
            let uuids: Vec<Uuid> = ids
                .iter()
                .map(|s| Uuid::parse_str(s).expect("Invalid UUID"))
                .collect();

            let peers = node
                .routing_engine
                .as_ref()
                .map(|engine| engine.server.get_connected_peers(&uuids))
                .unwrap_or_default();
            println!("Connected peers found: {}", peers.len());
            for p in peers {
                println!(
                    " - {} | {} | {}:{}",
                    p.node.name, p.node.id, p.node.address, p.node.port
                );
            }
        }

        PeerCommands::Add { name } => {
            let Some(&uuid) = known_nodes.get(&name) else {
                eprintln!("No node named '{}' found.", name);
                return;
            };
            if node.peers.contains(&uuid) {
                println!("{} already knows peer {}.", node.name, uuid);
            } else {
                node.peers.push(uuid);
                sync_node_engine_peers(node);
                println!("Peer '{}' ({}) added to {}.", name, uuid, node.name);
            }
        }

        PeerCommands::Remove { name } => {
            let Some(&uuid) = known_nodes.get(&name) else {
                eprintln!("No node named '{}' found.", name);
                return;
            };
            let before = node.peers.len();
            node.peers.retain(|p| *p != uuid);
            if node.peers.len() < before {
                sync_node_engine_peers(node);
                println!("Peer '{}' ({}) removed from {}.", name, uuid, node.name);
            } else {
                println!(
                    "Peer '{}' ({}) was not in {} peer list.",
                    name, uuid, node.name
                );
            }
        }
    }
}
