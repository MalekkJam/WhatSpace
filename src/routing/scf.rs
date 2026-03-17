use uuid::Uuid;
use chrono::Utc;
use crate::routing::model::{Bundle, MsgStatus};
use super::bundleManager::BundleManager;

// function called by the engine when the djikstra doesn't find the next hop
pub fn store(bundle: &mut Bundle, bundle_manager: &mut BundleManager) {
    // update the bundle status to pending before storing it
    bundle.shipment_status = MsgStatus::Pending;
    bundle_manager.storage.save_bundle(bundle);
}

// drop bundles that have exceeded their TTL
// function to be called at the start of the routing process to clean up expired bundles
pub fn drop_expired_bundles(bundle_manager: &mut BundleManager) {

    //TODO: fix after creating Bundle Manager and Storage Layer

    let now = Utc::now();

    //collection of the ids of the bundles that have expired (compare how old the bundle is with the bundle's ttl)
    let expired: Vec<String> = bundle_manager
        .all()
        .iter()
        .filter(|b| (now - b.timestamp).num_seconds() as u64 > b.ttl)
        .map(|b| b.id.clone())
        .collect();

    for id in expired {
        bundle_manager.delete_bundle(id);
    }
}

// function called when a contact opportunity comes up and returns bubdkes to forward to the next hop
// pub fn get_bundles_to_forward(bundle_manager: &mut BundleManager, next_hop: Uuid) -> Vec<Bundle> {
//     drop_expired_bundles(bundle_manager);
//     // résultat : bundle & next peer
//     // last line : call api to network
//     // anti _entropy() , find_next_hop()
// }