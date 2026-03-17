use uuid::Uuid; // id unique 
use serde::{Serialize, Deserialize};// for serializing and deserializing Rust data structures efficiently and generically, in the doc we can find the Derive Macros
use chrono::{DateTime, Utc}; // for the date and time 


// this file contains the data models 


// fot he node structure 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
        pub id: Uuid, // unique identifier for the node
        pub address: String, // IP address of the node
        pub port: u16, // port the node listens on
        pub peers: Vec<Uuid>, // IDs of known peer nodes
}

// implementation of the node struct
impl Node {
    pub fn new(address: &str, port: u16, peers: Vec<Uuid>) -> Self {
        Node {
            id: Uuid::new_v4(),
            address: address.to_string(),
            port,
            peers,
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
pub enum BundleKind{
    Data {msg: String}, // for the data bundle we need the message content
    Ack {ack_bundle_id: String}, // for the acknowledgment bundle we need the id of the bundle
}

//Bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: String,                  // id unique for the bundle
    pub source: Node,                // the source node of the bundle
    pub destination: Node,           // the destination node of the bundle
    pub timestamp: DateTime<Utc>,    // date and time of the bundle creation
    pub ttl: u64,                    // time to live in seconds, after which the bundle is considered expired
    pub kind: BundleKind,            // the kind of the bundle
    pub shipment_status: MsgStatus,  // the current status of the bundle during its lifecycle
}
//implementation of the bundle struct
impl Bundle {
    pub fn new(source: Node, destination: Node, kind: BundleKind, ttl: u64) -> Self { // for the new bundle we need the source, destination, kind and ttl 
        Bundle {
            id: Uuid::new_v4().to_string(), // generate a unique id for the bundle using uuid version 4 and convert it to string before storing it in the json file
                                            // more information inside the instructions.md file in the feat21-imple…D-generation section
            source,
            destination,
            timestamp: Utc::now(),
            ttl,
            kind,
            shipment_status: MsgStatus::Pending, //bydefault its pending when we create a new bundle
        }
    }
}
