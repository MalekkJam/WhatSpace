use std::io::{Read, Write};
use std::net::TcpStream;
use crate::routing::model::Node;
use serde_json::json;
use uuid::Uuid;
use std::collections::HashMap;
use crate::protobuf::bundle_proto::Bundle as ProtobufBundle;


fn getNode(node: &Node) -> Node {
    return node.clone(); //TODO: see if the clone method is good to get all the informations of the node 
                            // recupere une copie d ela node 
}

pub fn connect_to_server(node: &Node) -> Option<TcpStream> {
    // Get les informations  node
    let node_info = getNode(node);
    println!("Node récupéré: {:?}", node_info.id);
    
    // reate the tcp connection 
    let address = format!("{}:{}", node.address, node.port);
    match TcpStream::connect(&address) {
        Ok(mut stream) => {
            println!("Connecté à {}", address);
            
            //send infoirmations
            let message = json!({
                "id": node_info.id.to_string(),
                "address": node_info.address,
                "port": node_info.port,
                "peers": node_info.peers,
            })
            .to_string();
            
            
            match stream.write_all(message.as_bytes()) {
                Ok(_) => {
                    println!("Données envoyées au serveur !");
                    Some(stream)
                }
                Err(e) => {
                    println!("Erreur d'envoi: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            println!("Erreur de connexion: {}", e);
            None
        }
    }
}

//not sure about this fuction 
// to be corrected 
pub fn connect_to_peers(node: &Node,nodes_registry: &HashMap<Uuid, Node>,) -> Result<Vec<(Uuid, TcpStream)>, String> {

    // Verify that the registry is not empty
    if nodes_registry.is_empty() {
        return Err("Error: nodes registry is empty".to_string());
    }

    let mut connections = Vec::new();
    let mut errors = Vec::new(); 
    //todo: if this not necessary we can delete it 

    // Connect to all peers in the local node's peers list
    for &peer_id in &node.peers {
        match nodes_registry.get(&peer_id) {
            Some(peer) => {
                let address = format!("{}:{}", peer.address, peer.port);

                match TcpStream::connect(&address) {
                    Ok(mut stream) => {
                        println!("[Client] Connected to peer {} at {}", peer_id, address);

                        // Send node info to peer (Omar's function)
                        match send_info_to_client(&mut stream, node) {
                            Ok(info_map) => {
                                println!("[Client] Info sent to peer {}: {:?}", peer_id, info_map);
                                connections.push((peer_id, stream));
                            }
                            Err(e) => {
                                let msg = format!("Peer {} connected but info send failed: {}", peer_id, e);
                                println!("[Client] {}", msg);
                                errors.push(msg);
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!(
                            "Could not connect to peer {} at {}: {}",
                            peer_id, address, e
                        );
                        println!("[Client] {}", msg);
                        errors.push(msg); // we keep the error and continue trying to connect to other peers
                    }
                }
            }
            None => {
                let msg = format!("Peer {} not found in registry", peer_id);
                println!("[Client] {}", msg);
                errors.push(msg);
            }
        }
    }

    // TODO: if there is no connection BIG PROBLEM 
    if connections.is_empty() && !errors.is_empty() {
        return Err(format!(
            "Failed to connect to any peer. Errors:\n{}",
            errors.join("\n")
        ));
    }

    // THIS IS MIXED BUT IT CAN BE USEFUL TO KNOW WHICH PEERS FAILED TO CONNECT 
    // I AM NOT SURE IF THIS IS THE BEST WAY TO HANDLE THIS SITUATION
    if !errors.is_empty() {
        println!("[Client] Warning: some peers could not be reached:");
        for e in &errors {
            println!("  - {}", e);
        }
    }

    Ok(connections)
pub fn send_bundle(bundle: ProtobufBundle, peers: Vec<Node>) {

    // convert the bundle into the protobuf generated bundle
    //serialization of the protobuf to JSON string using protobuf-json-mapping
    let payload = match protobuf_json_mapping::print_to_string(&bundle) {
        Ok(json) => json.into_bytes(),
        Err(e) => {
            eprintln!("send_bundle failed to serialize bundle {} : {}", bundle.id, e);
            return;
        }
    };

    //iterate over eachh peer that the routing engine decided on
    for peer in peers {
        
        //build the peer address from the ip and port
        let address = format!("{}:{}", peer.address, peer.port);

        //Open a direct TCP connection to the peer
        let mut stream = match TcpStream::connect(address) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("send_bundle TCP connect to {} failed: {}", address, e);
                return;
            }
        };

        //sending with length prefix to let the receiver know exactly how many bytes to read
        let len = payload.len() as u32;
        if let Err(e) = stream
            .write_all(&len.to_be_bytes()) //writing the entire buffer to the tcp stream
            .and_then(|_| stream.write_all(&payload)) // this only runs if the previous is Ok
        {
            eprintln!("send_bundle failed to write to {}: {}", address, e);
            return;
        }

        //waiting for the ack peers
        let mut ack = [0u8; 4]; //buffer allocation
        match stream.read_exact(&mut ack) { //reads exactly 4 bytes from tcp stream into the ack buffer
            Ok(_) if &ack == b"ack\n" => {}
            Ok(_) => eprintln!("[send_bundle] unexpected ack from {}: {:?}", address, ack),
            Err(e) => eprintln!("[send_bundle] failed to read ack from {}: {}", address, e),
        }

    }

}