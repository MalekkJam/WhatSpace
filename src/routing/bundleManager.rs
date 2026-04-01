use crate::routing::model::{Bundle, BundleKind, MsgStatus};
use crate::storage::StorageLayer;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManager {
    pub node_id: Uuid,
    pub storage: StorageLayer,
}

impl BundleManager {
    // Function to get bundles stored at the node, used by the engine to get the summary vector

    pub fn new(node_id: Uuid, name: String) -> Self {
        let storage_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("bundles")
            .join(&name); // this anchors node storage under the project bundles directory regardless current working directory
        BundleManager {
            node_id,
            storage: StorageLayer::new(storage_dir.to_string_lossy().to_string(), 200), // this keeps bundle persistence consistent across interactive and one-shot terminal runs
        }
    }

    pub fn get_bundles_from_node(&self) -> Vec<Bundle> {
        self.storage.get_all_bundles()
    }

    // Function to get a bundle by its id, used by the SCF to fetch the full bundle before forwarding
    pub fn get(&self, bundle_id: Uuid) -> Option<Bundle> {
        self.storage.get_bundle(bundle_id)
    }

    // Function to delete a bundle by its id, used by the SCF to remove bundles that have been forwarded or expired
    pub fn delete_bundle(&mut self, bundle_id: Uuid) -> bool {
        self.storage.delete_bundle(bundle_id)
    }

    pub fn save_bundle(&mut self, bundle: &Bundle) -> bool {
        self.storage.save_bundle(bundle)
    }

    pub fn update_status(&mut self, bundle_id: Uuid, status: MsgStatus) -> bool {
        self.storage.update_bundle_status(bundle_id, status) // this delegates lifecycle status persistence to the storage layer
    }

    // Function to get all bundles stored at the node, used by the SCF to drop expired bundles
    pub fn all(&self) -> Vec<Bundle> {
        self.storage.get_all_bundles()
    }

    /// Called when an Ack bundle is received from a peer.
    /// Deletes the corresponding Data bundle from local storage.
    /// Returns false if the Ack was already known (duplicate).
    pub fn handle_incoming_ack(&mut self, ack: &Bundle) -> bool {
        // Deduplication — have we already seen this ACK?
        if self.storage.get_bundle(ack.id).is_some() {
            return false;
        }
        // Save the ACK to propagate it to other peers
        // Delete the corresponding local Data bundle
        if let BundleKind::Ack { ack_bundle_id } = &ack.kind {
            self.storage.delete_bundle(*ack_bundle_id);
            self.storage.save_bundle(ack) // always save ack so it continues to propagate
        } else {
            false
        }
    }

    /// Checks if a bundle is already known — used during anti-entropy
    /// to avoid resending bundles already present at a peer.
    pub fn has_bundle(&self, bundle_id: Uuid) -> bool {
        self.storage.get_bundle(bundle_id).is_some()
    }
}
