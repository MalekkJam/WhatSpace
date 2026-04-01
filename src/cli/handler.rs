use std::collections::{HashMap, HashSet}; // this import provides maps and sets used to track started nodes and open registry connections

use crate::cli::cli::{NodeCommands, PeerCommands}; // this import exposes parsed cli command enums
use crate::network::client::{connect_to_server, handle_peer_to_peer}; // this import provides registry registration and inbound peer handling helpers
use crate::network::server::{ServerRequest, ServerResponse}; // this import provides registry protocol messages used by cli peer queries
use crate::routing::model::{Bundle, BundleKind, Node}; // this import provides node and bundle models used by command handlers
use std::io::{Read, Write}; // this import provides stream io operations for registry peer queries
use std::net::TcpListener; // this import provides node listener binding for inbound bundle and anti-entropy requests
use std::net::TcpStream; // this import provides persistent registry stream handles for connection liveness
use std::sync::{Arc, Mutex, OnceLock}; // this import provides global synchronized state used by command handlers
use std::thread; // this import provides background listener threads per started node
use uuid::Uuid; // this import provides id parsing and map keys for nodes and peers

static REGISTRY_STREAMS: OnceLock<Mutex<HashMap<Uuid, TcpStream>>> = OnceLock::new(); // this global map keeps one registration stream per started node
static STARTED_NODES: OnceLock<Mutex<HashSet<Uuid>>> = OnceLock::new(); // this global set prevents duplicate listener startup for the same node

fn find_node<'a>(nodes: &'a [Node], name: &str) -> &'a Node {
    nodes.iter().find(|node| node.name == name).unwrap_or_else(|| {
        eprintln!("No node named '{}' found. Available nodes:", name); // this prints clear lookup failure details for user guidance
        for node in nodes {
            eprintln!("  - {}", node.name); // this prints valid node names to help user choose an existing node
        }
        std::process::exit(1); // this exits command immediately because continuing without a valid node would corrupt flow
    })
}

fn find_node_mut<'a>(nodes: &'a mut [Node], name: &str) -> &'a mut Node {
    if let Some(position) = nodes.iter().position(|node| node.name == name) {
        return &mut nodes[position]; // this returns mutable reference so command handlers can update node-local state
    }

    eprintln!("No node named '{}' found. Available nodes:", name); // this prints clear lookup failure details for user guidance
    for node in nodes.iter() {
        eprintln!("  - {}", node.name); // this prints valid node names to help user choose an existing node
    }
    std::process::exit(1); // this exits command immediately because continuing without a valid node would corrupt flow
}

pub async fn handle_command(command: NodeCommands, nodes: &mut Vec<Node>) {
    match command {
        NodeCommands::All => {
            if nodes.is_empty() {
                println!("No nodes found."); // this reports empty in-memory node list in one-shot or misconfigured runs
            } else {
                println!("Nodes in demo ({}):", nodes.len()); // this reports number of known nodes for quick operator visibility
                for node in nodes.iter() {
                    let started = STARTED_NODES
                        .get_or_init(|| Mutex::new(HashSet::new()))
                        .lock()
                        .map(|set| set.contains(&node.id))
                        .unwrap_or(false); // this checks if node listener was started in this process
                    println!(
                        "  - {} | {} | {}:{} | peers: {} | started: {}",
                        node.name,
                        node.id,
                        node.address,
                        node.port,
                        node.peers.len(),
                        started
                    ); // this prints full node summary including runtime start state
                }
            }
        }

        NodeCommands::Start { name, server } => {
            let node = find_node_mut(nodes, &name); // this resolves target node so startup can mutate shared engine state
            let bind_address = format!("{}:{}", node.address, node.port); // this builds listener endpoint for incoming tcp bundles and sv requests

            let routing_engine: Arc<Mutex<crate::routing::RoutingEngine>> = match &node.routing_engine {
                Some(engine) => Arc::clone(engine), // this clones shared routing engine handle for listener startup and command routing
                None => {
                    eprintln!("No routing engine available for {}.", node.name); // this reports missing runtime engine wiring on node object
                    return; // this aborts start because node cannot route without an engine
                }
            };

            {
                let mut guard = match routing_engine.lock() {
                    Ok(guard) => guard, // this acquires mutable engine access to update runtime config before listener starts
                    Err(e) => {
                        eprintln!("Failed to lock routing engine for {}: {}", node.name, e); // this logs lock poisoning issues to help recover startup
                        return; // this aborts startup because shared state cannot be safely mutated
                    }
                };
                guard.registry_addr = server.clone(); // this stores selected registry endpoint inside routing engine for peer discovery queries
                guard.peers = node.peers.clone(); // this synchronizes engine peer list with node peer configuration managed by cli
            }

            {
                let mut started = match STARTED_NODES
                    .get_or_init(|| Mutex::new(HashSet::new()))
                    .lock()
                {
                    Ok(started) => started, // this acquires mutable startup set to enforce one listener per node
                    Err(e) => {
                        eprintln!("Failed to lock started node set: {}", e); // this logs lock poisoning for startup guard failures
                        return; // this aborts startup when process state cannot be safely checked
                    }
                };

                if !started.contains(&node.id) {
                    match TcpListener::bind(&bind_address) {
                        Ok(listener) => {
                            println!("Node listener started on {}", bind_address); // this confirms listener startup endpoint for synchronization debugging
                            let engine_ref = Arc::clone(&routing_engine); // this clones shared routing engine so each inbound connection can route bundles
                            thread::spawn(move || {
                                for incoming in listener.incoming() {
                                    match incoming {
                                        Ok(stream) => {
                                            let engine_ref = Arc::clone(&engine_ref); // this clones engine handle for per-connection routing worker
                                            thread::spawn(move || {
                                                let mut guard = match engine_ref.lock() {
                                                    Ok(guard) => guard, // this acquires routing engine lock before processing inbound connection
                                                    Err(e) => {
                                                        eprintln!("Failed to lock routing engine in listener: {}", e); // this logs lock poisoning so listener failures are visible
                                                        return; // this aborts current connection handling when shared state is unavailable
                                                    }
                                                };
                                                handle_peer_to_peer(stream, &mut guard); // this delegates inbound peer traffic to existing network handler
                                            });
                                        }
                                        Err(e) => {
                                            eprintln!("Incoming listener error: {}", e); // this logs listener accept errors without stopping the node loop
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Failed to start listener for {} on {}: {}", node.name, bind_address, e); // this logs socket bind failures with node context
                            return; // this aborts registration because node cannot receive traffic without listener
                        }
                    }
                    started.insert(node.id); // this marks node as started to prevent duplicate listener binds
                }
            }

            let mut registration_node = node.clone(); // this clones node metadata for registry registration payload
            registration_node.routing_engine = None; // this strips runtime engine pointer before sending json registration to server

            let stream = connect_to_server(registration_node, Some(&server)); // this opens persistent registration stream to keep node marked connected
            if let Some(stream) = stream {
                if let Ok(mut streams) = REGISTRY_STREAMS
                    .get_or_init(|| Mutex::new(HashMap::new()))
                    .lock()
                {
                    streams.insert(node.id, stream); // this stores stream to keep connection alive until stop command drops it
                }
                println!("Node {} started and registered with server {}", node.name, server); // this confirms successful listener+registry startup
            } else {
                eprintln!("Failed to register node {} with server {}", node.name, server); // this reports registry registration failure after listener startup
            }
        }

        NodeCommands::Stop { name } => {
            let node = find_node(nodes, &name); // this resolves target node for shutdown semantics

            if let Ok(mut streams) = REGISTRY_STREAMS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
            {
                if streams.remove(&node.id).is_some() {
                    println!("Node {} disconnected from registry.", node.name); // this confirms registry disconnect so peers stop selecting this node
                } else {
                    println!("Node {} was not connected to registry.", node.name); // this reports no-op stop when node was not started
                }
            }

            if let Ok(mut started) = STARTED_NODES
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
            {
                started.remove(&node.id); // this allows listener restart in the same process after a stop command
            }
        }

        NodeCommands::Status { name } => {
            let node = find_node(nodes, &name); // this resolves node for status report
            let running = REGISTRY_STREAMS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .map(|streams| streams.contains_key(&node.id))
                .unwrap_or(false); // this derives connected state from active registry stream map

            let stored = node
                .routing_engine
                .as_ref()
                .and_then(|engine| engine.lock().ok().map(|guard| guard.bundle_manager.all().len()))
                .unwrap_or(0); // this reports local persisted bundle count from shared routing engine storage

            println!("ID : {}", node.id); // this prints node id for peer configuration debugging
            println!("Name : {}", node.name); // this prints human-readable node name
            println!("Address : {}:{}", node.address, node.port); // this prints listening endpoint used for peer connections
            println!("Peers : {}", node.peers.len()); // this prints locally configured peer count
            println!("Bundles : {}", stored); // this prints number of locally stored bundles
            println!("Running : {}", running); // this prints whether node is currently registered as connected
        }

        NodeCommands::Send {
            from,
            to,
            message,
            ttl,
        } => {
            let destination = find_node(nodes, &to).clone(); // this resolves destination before mutable sender borrow to satisfy borrow checker
            let sender = find_node_mut(nodes, &from); // this resolves mutable sender so route operation can update its engine state

            let mut bundle = Bundle::new(
                sender.clone(),
                destination,
                BundleKind::Data { msg: message },
                ttl,
            ); // this creates a new data bundle with pending status and sanitized source/destination runtime fields

            if let Some(engine) = &sender.routing_engine {
                let mut guard = match engine.lock() {
                    Ok(guard) => guard, // this acquires mutable engine for routing and anti-entropy operations
                    Err(e) => {
                        eprintln!("Failed to lock routing engine for {}: {}", sender.name, e); // this logs lock issues that prevent sending
                        return; // this aborts send when routing state is unavailable
                    }
                };
                guard.peers = sender.peers.clone(); // this synchronizes peer configuration before routing decision
                guard.route_bundle(&mut bundle).await; // this triggers store-carry-forward and epidemic forwarding workflow
            } else {
                eprintln!("No routing engine available for {}.", sender.name); // this reports configuration issue when sender has no engine
            }
        }

        NodeCommands::Peers { name, command } => {
            let known_nodes: HashMap<String, Uuid> =
                nodes.iter().map(|node| (node.name.clone(), node.id)).collect(); // this builds name-to-id lookup for add/remove peer commands
            let node = find_node_mut(nodes, &name); // this resolves mutable target node for peer list mutations

            handle_peer_command(command, node, &known_nodes); // this dispatches nested peer subcommands
        }

        #[cfg(feature = "debug")]
        NodeCommands::Debug { name } => match name {
            Some(name) => {
                let node = find_node(nodes, &name); // this resolves single node for debug json dump
                println!("{}", serde_json::to_string_pretty(node).unwrap()); // this prints full json for deep inspection during development
            }
            None => {
                println!("{}", serde_json::to_string_pretty(nodes).unwrap()); // this prints all nodes as json for development debugging
            }
        },
    }
}

fn handle_peer_command(command: PeerCommands, node: &mut Node, known_nodes: &HashMap<String, Uuid>) {
    match command {
        PeerCommands::ListPeers => {
            if node.peers.is_empty() {
                println!("No known peers for {}.", node.name); // this reports empty peer list when node is not configured yet
            } else {
                println!("Peers for {}:", node.name); // this labels peer list output for current node
                for peer in &node.peers {
                    println!("  - {}", peer); // this prints each known peer id
                }
            }
        }

        PeerCommands::GetConnectedPeers { ids } => {
            let uuids: Vec<Uuid> = ids
                .iter()
                .map(|value| Uuid::parse_str(value).expect("Invalid UUID"))
                .collect(); // this parses user-provided uuid strings for registry query

            let peers = node
                .routing_engine
                .as_ref()
                .and_then(|engine| {
                    engine.lock().ok().map(|guard| {
                        let mut result = Vec::new(); // this vector stores connected peers returned by the registry query
                        if let Ok(mut stream) = TcpStream::connect(&guard.registry_addr) {
                            let request = ServerRequest::GetConnectedPeers(uuids.clone()); // this asks registry for connected peers matching requested ids
                            if let Ok(payload) = serde_json::to_vec(&request) {
                                if stream.write_all(&payload).is_ok() {
                                    let mut buffer = [0u8; 16384]; // this buffer stores one registry response payload
                                    if let Ok(n) = stream.read(&mut buffer) {
                                        if n > 0 {
                                            if let Ok(ServerResponse::Peers(peers)) =
                                                serde_json::from_slice::<ServerResponse>(&buffer[..n])
                                            {
                                                result = peers
                                                    .into_iter()
                                                    .filter(|peer| peer.node.id != guard.node_id)
                                                    .collect(); // this removes self from result list so output stays meaningful
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        result
                    })
                })
                .unwrap_or_default(); // this queries registry for currently connected peers among requested ids

            println!("Connected peers found: {}", peers.len()); // this prints number of matching connected peers returned by server
            for peer in peers {
                println!(
                    " - {} | {} | {}:{}",
                    peer.node.name, peer.node.id, peer.node.address, peer.node.port
                ); // this prints each connected peer identity and endpoint
            }
        }

        PeerCommands::Add { name } => {
            let Some(&uuid) = known_nodes.get(&name) else {
                eprintln!("No node named '{}' found.", name); // this reports invalid peer name references to prevent bad ids in peer list
                return;
            };
            if node.peers.contains(&uuid) {
                println!("{} already knows peer {}.", node.name, uuid); // this reports idempotent add when peer already exists
            } else {
                node.peers.push(uuid); // this inserts new peer id into local peer configuration
                println!("Peer '{}' ({}) added to {}.", name, uuid, node.name); // this confirms peer add mutation for operator feedback
            }
        }

        PeerCommands::Remove { name } => {
            let Some(&uuid) = known_nodes.get(&name) else {
                eprintln!("No node named '{}' found.", name); // this reports invalid peer name references to prevent silent no-ops
                return;
            };
            let before = node.peers.len(); // this stores pre-removal length to detect whether a peer was actually removed
            node.peers.retain(|peer| *peer != uuid); // this removes matching peer ids while preserving all other entries
            if node.peers.len() < before {
                println!("Peer '{}' ({}) removed from {}.", name, uuid, node.name); // this confirms successful peer removal
            } else {
                println!(
                    "Peer '{}' ({}) was not in {} peer list.",
                    name, uuid, node.name
                ); // this reports no-op remove when peer was absent
            }
        }
    }
}
