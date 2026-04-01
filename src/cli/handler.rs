use uuid::Uuid;

use crate::cli::nodeCli::{NodeCommands, PeerCommands};
use crate::network::client::connect_to_server;
use crate::routing::model::{Bundle, BundleKind, Node};


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
    if let Some(index) = nodes.iter().position(|n| n.name == name) {
        return &mut nodes[index];
    }

    eprintln!("No node named '{}' found. Available nodes:", name);
    for n in nodes.iter() {
        eprintln!("  - {}", n.name);
    }
    std::process::exit(1);
}

pub async fn handle_command(command: NodeCommands, nodes: &mut Vec<Node>) {
    // Keep storage clean on every command cycle.
    for node in nodes.iter_mut() {
        if let Some(engine) = node.routing_engine.as_mut() {
            engine.bundle_manager.cleanup_expired_bundles();
        }
    }

    match command {
        NodeCommands::All => {
            if nodes.is_empty() {
                println!("No nodes found.");
            } else {
                println!("Nodes in demo ({}):", nodes.len());
                for node in nodes.iter() {
                    println!(
                        "  - {} | {} | {}:{} | peers: {}",
                        node.name, node.id, node.address, node.port, node.peers.len()
                    );
                }
            }
        }

        NodeCommands::Start { name, server } => {
            let node = find_node(nodes, &name);

            // just register with the registry server
            let connected = connect_to_server(node.clone(), &server);
            if !connected {
                eprintln!("Failed to connect node {} to server", node.name);
                return;
            }
            println!("Node {} registered with server {}", node.name, server);
        }

        NodeCommands::Stop { name } => {
            let node = find_node(nodes, &name);

            println!("Stopping node {}...", node.name);

            if let Some(engine) = node.routing_engine.as_ref() {
                engine.server.disconnect_server();
            } else {
                eprintln!("Node {} has no routing engine", node.name);
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

        NodeCommands::Send { from, to, message, ttl } => {
            if from == to {
                eprintln!("Sender and destination must be different nodes.");
                return;
            }

            let sender_idx = nodes.iter().position(|n| n.name == from).unwrap_or_else(|| {
                eprintln!("No node named '{}' found.", from);
                std::process::exit(1);
            });
            let dest_idx = nodes.iter().position(|n| n.name == to).unwrap_or_else(|| {
                eprintln!("No node named '{}' found.", to);
                std::process::exit(1);
            });

            let (sender, destination) = if sender_idx < dest_idx {
                let (left, right) = nodes.split_at_mut(dest_idx);
                (&mut left[sender_idx], &mut right[0])
            } else {
                let (left, right) = nodes.split_at_mut(sender_idx);
                (&mut right[0], &mut left[dest_idx])
            };

            let mut bundle = Bundle::new(
                sender.clone(),
                destination.clone(),
                BundleKind::Data { msg: message },
                ttl,
            );

            // Source persists and forwards.
            if let Some(engine) = sender.routing_engine.as_mut() {
                engine.route_bundle(&mut bundle).await;
            }

            // Simulate destination reception in this interactive single-process demo.
            if let Some(engine) = destination.routing_engine.as_mut() {
                engine.route_bundle(&mut bundle).await;
            }

            // Auto-generate ACK and route it back to source in-session.
            let mut ack = Bundle::new_ack(&bundle);
            if let Some(engine) = sender.routing_engine.as_mut() {
                engine.route_bundle(&mut ack).await;
            }
        }

        NodeCommands::Peers { name, command } => {
            let node_idx = nodes.iter().position(|n| n.name == name).unwrap_or_else(|| {
                eprintln!("No node named '{}' found. Available nodes:", name);
                for n in nodes.iter() {
                    eprintln!("  - {}", n.name);
                }
                std::process::exit(1);
            });

            let mut node = nodes.remove(node_idx);
            handle_peer_command(command, &mut node, nodes);
            nodes.insert(node_idx, node);
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

fn handle_peer_command(command: PeerCommands, node: &mut Node, nodes: &[Node]) {
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

            if let Some(engine) = node.routing_engine.as_ref() {
                let peers = engine.server.get_connected_peers(&uuids);
                println!("Connected peers found: {}", peers.len());
                for p in peers {
                    println!(
                        " - {} | {} | {}:{}",
                        p.node.name, p.node.id, p.node.address, p.node.port
                    );
                }
            } else {
                eprintln!("Node {} has no routing engine", node.name);
            }
        }

        PeerCommands::Add { name } => {
            let peer = find_node(nodes, &name);
            let uuid = peer.id;
            if node.peers.contains(&uuid) {
                println!("{} already knows peer {}.", node.name, uuid);
            } else {
                node.peers.push(uuid);
                println!("Peer {} added to {}.", uuid, node.name);
            }
        }

        PeerCommands::Remove { name } => {
            let peer = find_node(nodes, &name);
            let uuid = peer.id;
            let before = node.peers.len();
            node.peers.retain(|p| *p != uuid);
            if node.peers.len() < before {
                println!("Peer {} removed from {}.", uuid, node.name);
            } else {
                println!("Peer {} was not in {} peer list.", uuid, node.name);
            }
        }
    }
}