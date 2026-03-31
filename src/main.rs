mod network;
mod routing;
mod storage;
use crate::network::protobuf::{deserialize,serialize};
use crate::routing::model::{Bundle, BundleKind, MsgStatus, Node};
use crate::storage::StorageLayer;
use chrono::Utc;
use uuid::Uuid;


fn main() {
    // creer le,ficheir de storage automatiquement au lancer de l'app

    test_protobuf();
}

fn test_protobuf() {
    let node_a = Node {
        id: Uuid::new_v4(),
        name: "node_a".to_string(),
        address: "127.0.0.1".to_string(),
        port: 8081,
        peers: vec![],
    };

    let node_b = Node {
        id: Uuid::new_v4(),
        name: "node_b".to_string(),
        address: "127.0.0.1".to_string(),
        port: 8082,
        peers: vec![],
    };

    let bundle = Bundle {
        id: Uuid::new_v4(),
        source: node_a,
        destination: node_b,
        timestamp: Utc::now(),
        ttl: 3600,
        kind: BundleKind::Data {
            msg: "hello from protobuf test".to_string(),
        },
        shipment_status: MsgStatus::Pending,
    };

    println!("Original bundle id: {}", bundle.id);

    // serialize
    let proto_bundle = crate::network::bundle::ProtobufBundle::from(&bundle);
    let bytes = match serialize(&proto_bundle) {
        Some(b) => b,
        None => {
            println!("serialization failed");
            return;
        }
    };

    println!("Serialized {} bytes", bytes.len());

    // deserialize
    let recovered = match deserialize(&bytes) {
        Some(b) => Bundle::from(b),
        None => {
            println!("deserialization failed");
            return;
        }
    };

    println!("Recovered bundle id: {}", recovered.id);
    println!("Payload: {:?}", recovered.kind);
    println!("Protobuf test OK!");
}
