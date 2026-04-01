use std::collections::HashMap;
use std::io::{Read, Write};

use crate::cli::cli::{NodeCommands, PeerCommands};
use crate::network::client::{connect_to_server, receive_bundle};
use crate::routing::bundleManager::BundleManager;
use crate::routing::model::{Bundle, BundleKind, Node};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use uuid::Uuid;

// Vec pour garder TOUTES les connexions actives (une par nœud démarré).
// Option<TcpStream> était écrasée à chaque `start`, déconnectant les nœuds précédents.
static REGISTRY_STREAMS: Mutex<Vec<TcpStream>> = Mutex::new(Vec::new());

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
            let node = find_node(nodes, &name).clone();
            let stream = connect_to_server(node.clone());
            if stream.is_none() {
                eprintln!("Failed to connect node {} to server", node.name);
                return;
            }
            // Ajoute la connexion au Vec — ne remplace plus les connexions existantes
            REGISTRY_STREAMS.lock().unwrap().push(stream.unwrap());
            println!("Node {} registered with server {}", node.name, server);

            // Démarre un listener P2P sur le port du nœud pour recevoir les bundles
            // ET répondre aux RequestSV (anti-entropy). Un thread par connexion
            // pour éviter qu'une requête bloquante ne gèle tout le listener.
            let node_name = node.name.clone();
            let node_id   = node.id;
            let node_port = node.port;
            std::thread::spawn(move || {
                let listener = match TcpListener::bind(format!("127.0.0.1:{}", node_port)) {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[{}] P2P listen failed on port {}: {}", node_name, node_port, e);
                        return;
                    }
                };
                println!("[{}] P2P listener started on 127.0.0.1:{}", node_name, node_port);
                for incoming in listener.incoming() {
                    match incoming {
                        Ok(s) => {
                            let nn = node_name.clone();
                            let ni = node_id;
                            std::thread::spawn(move || {
                                handle_p2p_connection(s, ni, nn);
                            });
                        }
                        Err(e) => eprintln!("[{}] P2P accept error: {}", node_name, e),
                    }
                }
            });
        }

        NodeCommands::Stop { name } => {
            let node = find_node(nodes, &name);

            println!("Stopping node {}...", node.name);

            if let Some(engine) = &node.routing_engine {
                engine.server.disconnect_server();
            } else {
                eprintln!("No routing engine available for {}.", node.name);
            }
        }

        NodeCommands::Status { name } => {
            let node = find_node(nodes, &name);
            let stored = node
                .routing_engine
                .as_ref()
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

/// Gère une connexion P2P entrante.
/// Peek le premier octet pour distinguer :
///'{'message JSON de contrôle (RequestSV) on répond avec le SummaryVector
///autre bundle protobuf (length-prefix) on reçoit et on sauvegarde
fn handle_p2p_connection(mut stream: TcpStream, node_id: Uuid, node_name: String) {
    let mut first = [0u8; 1];
    match stream.peek(&mut first) {
        Ok(0) | Err(_) => return,
        _ => {}
    }

    if first[0] == b'{' {
        // ── RequestSV (JSON) ──────────────────────────────────────
        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(n) if n > 0 => n,
            _ => return,
        };
        if let Ok(BundleKind::RequestSV { from }) = serde_json::from_slice::<BundleKind>(&buf[..n])
        {
            eprintln!("[{}] RequestSV from {}", node_name, from);
            let bm = BundleManager::new(node_id, node_name.clone());
            let ids: Vec<Uuid> = bm.get_bundles_from_node().iter().map(|b| b.id).collect();
            let resp = BundleKind::SummaryVector { ids };
            if let Ok(payload) = serde_json::to_vec(&resp) {
                let _ = stream.write_all(&payload);
            }
        }
    } else {
        // ── Bundle protobuf (length-prefix) ──────────────────────
        let mut bm = BundleManager::new(node_id, node_name.clone());
        if let Some(mut bundle) = receive_bundle(&mut stream) {
            // Applique une transition de statut au moment de la réception.
            if bundle.is_expired() {
                bundle.shipment_status = crate::routing::model::MsgStatus::Expired;
            } else if bundle.destination.id == node_id {
                bundle.shipment_status = crate::routing::model::MsgStatus::Delivered;
            } else {
                bundle.shipment_status = crate::routing::model::MsgStatus::InTransit;
            }

            eprintln!(
                "[{}] bundle {} received, status -> {:?}",
                node_name, bundle.id, bundle.shipment_status
            );

            // Upsert pour éviter de rester bloqué en Pending en cas de doublon.
            if !bm.save_bundle(&bundle) {
                let _ = bm.update_bundle(&bundle);
            }
        }
    }
}
