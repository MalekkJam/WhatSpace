use chrono::Utc;
use uuid::Uuid;

use super::model::{Bundle, BundleKind, MsgStatus};

impl Bundle{
    /// Create an Ack bundle from a successfully delivered Data bundle.
    /// Called by the destination node upon receiving a Data bundle
    /// intended for it. Source and destination are automatically swapped.
    pub fn new_ack(delivered_bundle: &Bundle) -> Self {
        let mut ack_source = delivered_bundle.destination.clone(); // this clones destination node because it becomes ack source
        ack_source.routing_engine = None; // this strips runtime engine pointer from wire/storage payload
        let mut ack_destination = delivered_bundle.source.clone(); // this clones original source because it becomes ack destination
        ack_destination.routing_engine = None; // this strips runtime engine pointer from wire/storage payload

        Bundle {
            id: Uuid::new_v4(),
            // The destination of the Data becomes the source of the Ack
            source: ack_source,
            // The source of the Data becomes the destination of the Ack
            destination: ack_destination,
            timestamp: Utc::now(),
            // Same TTL as the original so the Ack has time to propagate back
            ttl: delivered_bundle.ttl,
            // Ack bundles carry no payload,
            kind: BundleKind::Ack {
                ack_bundle_id: delivered_bundle.id.clone(),
            },
            shipment_status: MsgStatus::Pending,
        }
    }
}


