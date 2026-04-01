use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize}; // for serializing and deserializing Rust data structures efficiently and generically, in the doc we can find the Derive Macros
use std::sync::{Arc, Mutex}; // this import enables shared mutable routing engine access across cli and listener threads
use uuid::Uuid;

use crate::routing::RoutingEngine; // this model only needs the routing engine type for the optional node field

// this file contains the data models

// fot he node structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid, // unique identifier for the node
    pub name: String,
    pub address: String,  // IP address of the node
    pub port: u16,        // port the node listens on
    pub peers: Vec<Uuid>, // IDs of known peer nodes
    #[serde(skip)]
    pub routing_engine: Option<Arc<Mutex<RoutingEngine>>>, // this shared engine instance keeps cli commands and listener processing in one consistent state
}

// implementation of the node struct
impl Node {
    pub fn new(name: &str, address: &str, port: u16, peers: Vec<Uuid>) -> Self {
        let new_id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, name.as_bytes()); // derive a stable id from the node name so separate processes agree on peer identities
        Node {
            id: new_id,
            name: name.to_string(),
            address: address.to_string(),
            port,
            peers: peers.clone(),
            routing_engine: Some(Arc::new(Mutex::new(RoutingEngine::new(
                new_id,
                peers,
                name.to_string(),
            )))), // this wraps the routing engine in Arc<Mutex<_>> so multiple threads can mutate one source of truth
        }
    }
}

// fot the MsgStatus we use an enumeration to represent the different status of the bundle during its lifecycle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MsgStatus {
    // the bundle is created but not yet sent
    Pending,

    // the bundle is on the way to the destination
    InTransit,

    // the bundle has been delivered to the destination
    /// For Data bundles: set when an Ack is received, then deleted from storage.
    /// For Ack bundles: set when the Ack reaches the original sender.
    Delivered,

    // the bundle has expired //TTL exceeded
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BundleKind {
    Data { msg: String }, // for the data bundle we need the message content
    Ack { ack_bundle_id: Uuid },
    // new: A asks B for its summary vector
    RequestSV { from: Uuid },
    // new: B replies with its list of bundle IDs
    SummaryVector { ids: Vec<Uuid> }, // for the acknowledgment bundle we need the id of the bundle
}

//Bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: Uuid,                   // id unique for the bundle
    pub source: Node,               // the source node of the bundle
    pub destination: Node,          // the destination node of the bundle
    pub timestamp: DateTime<Utc>,   // date and time of the bundle creation
    pub ttl: u64, // time to live in seconds, after which the bundle is considered expired
    pub kind: BundleKind, // the kind of the bundle
    pub shipment_status: MsgStatus, // the current status of the bundle during its lifecycle
}
//implementation of the bundle struct
impl Bundle {
    pub fn new(source: Node, destination: Node, kind: BundleKind, ttl: u64) -> Self {
        let mut source = source; // this creates an owned mutable source node to sanitize non-serializable runtime fields
        source.routing_engine = None; // this avoids recursively embedding runtime engines into bundle payloads and storage snapshots
        let mut destination = destination; // this creates an owned mutable destination node to sanitize runtime fields
        destination.routing_engine = None; // this keeps bundle payload lightweight and deterministic across processes

        // for the new bundle we need the source, destination, kind and ttl
        Bundle {
            id: Uuid::new_v4(), // generate a unique id for the bundle using uuid version 4 and convert it to string before storing it in the json file
            // more information inside the instructions.md file in the feat21-imple…D-generation section
            source,
            destination,
            timestamp: Utc::now(),
            ttl,
            kind,
            shipment_status: MsgStatus::Pending, //bydefault its pending when we create a new bundle
        }
    }

    // Returns true if this bundle has exceeded its TTL.
    pub fn is_expired(&self) -> bool {
        let age = Utc::now()
            .signed_duration_since(self.timestamp)
            .num_seconds();
        age > self.ttl as i64
    }
}
