use uuid::Uuid; // id unique 
use serde::{Serialize, Deserialize};// for serializing and deserializing Rust data structures efficiently and generically, in the doc we can find the Derive Macros
use chrono::{DateTime, Utc}; // for the date and time 


// this file contains the data models 


// fot he node structure 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,      // string names ad example "NodeA" or "NodeB"
    pub address: String, // the adresse of the node 
    //TODO : is it an IP address ? or a name ?

}

// implementation of the node struct
impl Node {
    pub fn new(id: &str, address: &str) -> Self {
        Node {
            id: id.to_string(),
            address: address.to_string(),
        }
    }
}

// fot the MsgStatus we use an enumeration to represent the different status of the bundle during its lifecycle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MsgStatus { // TODO: is it correct for the moment to use these for the status of the bundle ?
    Pending,    // the bundle is created but not yet sent
    InTransit,  // the bundle is on the way to the destination
    Delivered,  // the bundle has been delivered to the destination
    Expired,    // the bundle has expired //TTL exceeded
}

//Bundle 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: String,                  // id unique for the bundle
    pub source: Node,                // the source node of the bundle
    pub destination: Node,           // the destination node of the bundle
    pub timestamp: DateTime<Utc>,    // date and time of the bundle creation
    pub ttl: u64,                    // time to live in seconds, after which the bundle is considered expired
    //TODO: is it correct to use u64 for the ttl ? or should we use a different type ?
    pub msg: String,                 // the message content of the bundle
    pub shipment_status: MsgStatus,  // the current status of the bundle during its lifecycle
}
//implementation of the bundle struct
impl Bundle {
    pub fn new(source: Node, destination: Node, msg: String, ttl: u64) -> Self { // for the new bundle we need the source, destination, message and ttl 
        //TODO: is it correct htat i added the ttl for the time to live of the bundle or we don't need it in the structure 
        Bundle {
            id: Uuid::new_v4().to_string(), // generate a unique id for the bundle using uuid version 4 and convert it to string before storing it in the json file
                                            // more information inside the instructions.md file in the feat21-imple…D-generation section
            source,
            destination,
            timestamp: Utc::now(),
            ttl,
            msg,
            shipment_status: MsgStatus::Pending, //bydefault its pending when we create a new bundle
        }
    }
}
