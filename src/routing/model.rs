use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize}; 
use uuid::Uuid;

use crate::routing::RoutingEngine; 



// fot he node structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid, // unique identifier for the node
    pub name: String,
    pub address: String,  // IP address of the node
    pub port: u16,        // port the node listens on
    pub peers: Vec<Uuid>, // IDs of known peer nodes
    #[serde(skip)]
    pub routing_engine: Option<RoutingEngine>, // cause we do not want to initialize the routing_engine when we initialize  the source and destination in the Bundle Struct
}

// implementation of the node struct
impl Node {
    pub fn new(name: &str, address: &str, port: u16, peers: Vec<Uuid>) -> Self {
        let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
        let id = Uuid::new_v5(&namespace, name.as_bytes());
        Node {
            id,
            name: name.to_string(),
            address: address.to_string(),
            port,
            peers: peers.clone(),
            routing_engine: Some(RoutingEngine::new(id, peers, name.to_string())),
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
        Bundle {
            id: Uuid::new_v4(), // generate a unique id for the bundle using uuid version 4 and convert it to string before storing it in the json file
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
