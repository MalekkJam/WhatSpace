use crate::protobuf::bundle_proto::Bundle as ProtobufBundle;
use crate::routing::model::Node;
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;

fn getNode(node: &Node) -> Node {
    return node.clone(); //TODO: see if the clone method is good to get all the informations of the node
                         // recupere une copie d ela node
}

pub fn connect_to_server(node: &Node) -> bool {
    // Get les informations  node
    let node_info = getNode(node);
    println!("Node récupéré: {:?}", node_info.id);

    // reate the tcp connection
    let address = format!("{}:{}", node.address, node.port);
    match TcpStream::connect(&address) {
        Ok(mut stream) => {
            println!("Connecté à {}", address);

            //send informations
            let message = json!({ServerRequest::Register : {
                "id": node_info.id.to_string(),
                "address": node_info.address,
                "port": node_info.port,
                "peers": node_info.peers,
            }})
            .to_string();

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
            None
        }
    }
}

pub fn connect_to_peer(origin: &Node, destination: &Node) -> Option<TcpStream> {
    let addr = format!("{}:{}", destination.address, destination.port);
    match TcpStream::connect(&addr) {
        Ok(stream) => {
            println!("[{}] Connected to peer at {}", origin.id, addr);
            Some(stream)
        }
        Err(e) => {
            eprintln!("[{}] Failed to connect to {}: {}", origin.id, addr, e);
            None
        }
    }
}

pub fn connect_to_peers(node: &Node) -> Result<Vec<(Uuid, TcpStream)>, String> {
    let connected_peers = get_connected_peers(node);

    if connected_peers.is_empty() {
        return Err("no connected peers yet".to_string());
    }

    let streams: Vec<(Uuid, TcpStream)> = connected_peers
        .into_iter()
        .filter_map(|(id, (port, address))| {
            let dest = Node { id, address, port, ..Default::default() };
            connect_to_peer(node, &dest).map(|stream| (id, stream))
        })
        .collect();

    if streams.is_empty() {
        return Err("could not connect to any peer".to_string());
    }

    Ok(streams)
}

pub fn send_bundle(bundle: ProtobufBundle, peers: Vec<Node>) {
    // convert the bundle into the protobuf generated bundle
    //serialization of the protobuf to JSON string using protobuf-json-mapping
    let payload = match protobuf_json_mapping::print_to_string(&bundle) {
        Ok(json) => json.into_bytes(),
        Err(e) => {
            eprintln!(
                "send_bundle failed to serialize bundle {} : {}",
                bundle.id, e
            );
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
            .and_then(|_| stream.write_all(&payload))
        // this only runs if the previous is Ok
        {
            eprintln!("send_bundle failed to write to {}: {}", address, e);
            return;
        }

        //waiting for the ack peers
        let mut ack = [0u8; 4]; //buffer allocation
        match stream.read_exact(&mut ack) {
            //reads exactly 4 bytes from tcp stream into the ack buffer
            Ok(_) if &ack == b"ack\n" => {}
            Ok(_) => eprintln!("[send_bundle] unexpected ack from {}: {:?}", address, ack),
            Err(e) => eprintln!("[send_bundle] failed to read ack from {}: {}", address, e),
        }
    }
}
