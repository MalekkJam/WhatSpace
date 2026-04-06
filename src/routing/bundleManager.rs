use crate::routing::model::{Bundle, BundleKind, MsgStatus};
use crate::storage::StorageLayer;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManager {
    pub node_id: Uuid,
    pub storage: StorageLayer,
}

impl BundleManager {
    // Function to get bundles stored at the node, used by the engine to get the summary vector

    pub fn new(node_id: Uuid, name: String) -> Self {
        BundleManager {
            node_id,
            storage: StorageLayer::new(format!("./bundles/{}", name), 100),
        }
    }

    pub fn get_bundles_from_node(&mut self) -> Vec<Bundle> {
        self.storage.get_all_bundles()
    }

    // Function to get a bundle by its id, used by the SCF to fetch the full bundle before forwarding
    pub fn get(&mut self, bundle_id: Uuid) -> Option<Bundle> {
        self.storage.get_bundle(bundle_id)
    }

    // Function to delete a bundle by its id, used by the SCF to remove bundles that have been forwarded or expired
    pub fn delete_bundle(&mut self, bundle_id: Uuid) -> bool {
        self.storage.delete_bundle(bundle_id)
    }

    pub fn save_bundle(&mut self, bundle: &Bundle) -> bool {
        self.storage.save_bundle(bundle)
    }

    pub fn upsert_bundle(&mut self, bundle: &Bundle) -> bool {
        if self.storage.get_bundle(bundle.id).is_some() {
            self.storage.update_bundle(bundle)
        } else {
            self.storage.save_bundle(bundle)
        }
    }

    // Function to get all bundles stored at the node, used by the SCF to drop expired bundles
    pub fn all(&mut self) -> Vec<Bundle> {
        self.storage.get_all_bundles()
    }

    pub fn has_ack_for_data_bundle(&mut self, data_bundle_id: Uuid) -> bool {
        self.storage.get_all_bundles().iter().any(|b| match &b.kind {
            BundleKind::Ack { ack_bundle_id } => *ack_bundle_id == data_bundle_id,
            _ => false,
        })
    }

    /// Called when an Ack bundle is received from a peer.
    /// Marks the corresponding Data bundle as Delivered and then deletes it.
    /// Returns false if the Ack was already known (duplicate).
    pub fn handle_incoming_ack(&mut self, ack: &Bundle) -> bool {
        if let BundleKind::Ack { ack_bundle_id } = &ack.kind {
            // Always apply local cleanup for the matching DATA bundle if present.
            if let Some(mut data_bundle) = self.storage.get_bundle(*ack_bundle_id) {
                data_bundle.shipment_status = MsgStatus::Delivered;
                let _ = self.storage.update_bundle(&data_bundle);
                let _ = self.storage.delete_bundle(*ack_bundle_id);
            }

            // Deduplicate ACK persistence/forwarding.
            if self.has_ack_for_data_bundle(*ack_bundle_id) || self.storage.get_bundle(ack.id).is_some() {
                return false;
            }

            // Keep one ACK record per data bundle (ACK id is deterministic).
            self.storage.save_bundle(ack)
        } else {
            false
        }
    }

    /// Checks if a bundle is already known — used during anti-entropy
    /// to avoid resending bundles already present at a peer.
    pub fn has_bundle(&mut self, bundle_id: Uuid) -> bool {
        self.storage.get_bundle(bundle_id).is_some()
    }
}
