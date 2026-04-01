use super::bundleManager::BundleManager;
use super::engine::RoutingEngine;
use crate::routing::model::{Bundle, MsgStatus};
use std::time::Duration;
use uuid::Uuid;

// function called by the engine when the djikstra doesn't find the next hop
pub fn store(bundle: &mut Bundle, bundle_manager: &mut BundleManager) {
    // update the bundle status to pending before storing it
    bundle.shipment_status = MsgStatus::Pending;
    bundle_manager.save_bundle(bundle);
}

impl RoutingEngine {
    // drop bundles that have exceeded their TTL
    // function to be called at the start of the routing process to clean up expired bundles
    pub fn drop_expired_bundles(&self, bundle_manager: &mut BundleManager) {
        let expired: Vec<Uuid> = bundle_manager
            .all()
            .iter()
            .filter(|b| b.is_expired())
            .map(|b| b.id)
            .collect();

        for id in expired {
            bundle_manager.delete_bundle(id);
        }
    }

    pub async fn forward_loop(&self, bundle_manager: &mut BundleManager, retry_interval : Duration) {
        self.drop_expired_bundles(bundle_manager);
        tokio::time::sleep(retry_interval).await;
    }
}
