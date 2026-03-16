use super::epidemic::NetworkGraph;
use chrono::offset::Utc;
use pathfinding::directed::dijkstra::dijkstra;
use uuid::Uuid;

pub struct RoutingEngine {
    pub node_id: Uuid,
    pub graph: NetworkGraph, // for Dijkstra
}

impl RoutingEngine {
    // Summary vector management
    pub fn get_summary_vector(&self, bundle_manager: &BundleManager) -> Vec<Uuid> {
        return bundle_manager.get_bundles_from_node(self.node_id); // this function calls the storage layer to get the bundles stored
    }

    pub fn anti_entropy(&self, local_sv: &[Uuid], peer_sv: &[Uuid]) -> Vec<Uuid> {
        // compare local_sv with peer_sv and at the end peer_sv should be equal to local_sv in terms of content
        let mut missing_on_peer: Vec<Uuid> = vec![];
        for &i in local_sv.iter() {
            if !peer_sv.contains(&i) {
                missing_on_peer.push(i);
            }
        }
        missing_on_peer
    }

    // Dijkstra to find next hop
    pub fn find_next_hop(&self, destination: Uuid) -> Option<Uuid> {
        let (path, _) = dijkstra(
            &self.node_id,
            |node| self.graph.neighbors(node),
            |node| *node == destination,
        )?;
        path.get(1).copied()
    }

    // Main routing decision
    pub fn route_bundle(
        &self,
        bundle: &Bundle,
        bundle_manager: &BundleManager,
        network: &NetworkLayer,
    ) {
        // Check if we are the destination
        if self.node_id == bundle.destination {
            // TODO: update bundle status to delivered
            // wainting for issue #26
            return;
        }

        // Check if TTL expired
        let elapsed = Utc::now() - bundle.timestamp;
        if elapsed.num_seconds() as u64 > bundle.ttl {
            // issue #27
            bundle_manager.delete_bundle(bundle.id);
            return;
        }

        // Find next hop using Dijkstra
        let next_hop = self.find_next_hop(bundle.destination);
        if next_hop.is_none() {
            // waiting for issue #22
            bundle_manager.store_bundle(bundle);
            return;
        }

        // Get local summary vector
        // issue #27
        let local_sv = self.get_summary_vector(bundle_manager);

        // Get peer summary vector
        // issue #28
        let peer_sv = network.get_peer_summary_vector(next_hop.unwrap());

        // waiting for issue #24
        let to_send = self.anti_entropy(&local_sv, &peer_sv);

        // Check for duplicates before sending
        // waiting for issue #25 but already taken into consideration in the anti_entropy function

        // issue #28
        network.send_bundles(next_hop.unwrap(), to_send);
    }
}
