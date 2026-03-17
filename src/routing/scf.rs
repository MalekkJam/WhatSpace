use uuid::Uuid;
use chrono::Utc;
use crate::routing::model::{Bundle, MsgStatus};
use super::bundleManager::BundleManager;
use super::engine::RoutingEngine;

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
pub fn get_bundles_to_forward(bundle_manager: &mut BundleManager, next_hop: Uuid) -> Vec<Bundle> {
    drop_expired_bundles(bundle_manager);
    // résultat : bundle & next peer
    // last line : call api to network
    // anti _entropy() , find_next_hop()

    let engine = RoutingEngine {
        node_id: bundle_manager.node_id,
        graph: NetworkGraph::new(), // TODO: pass the actual graph
    };

    // delegation of the path decision to the engine
    let Some(next_hop) = engine.find_next_hop(next_hop) else {
        // if no next hop is found, we store the bundle for later forwarding
        return vec![];
    };

    // using the anti_entropy function to compare 2 summary vectors and return what the peer is missing
    let candidates: Vec<Uuid> = engine.anti_entropy(
        &bundle_manager.get_bundles_from_node(bundle_manager.node_id),
        &bundle_manager.get_bundles_from_node(next_hop),
    );

    //fetching each bundle by id and returning the bundles to forward
    candidates
        .into_iter()
        .filter_map(|id| {
            let _id = id.to_string();
            bundle_manager.get(&_id)
        })
        .collect()    
}