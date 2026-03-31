use super::bundleManager::BundleManager;
use super::epidemic::NetworkGraph;
use super::model::Bundle;
use crate::network::client::send_bundle;
use crate::network::server::Server;
use crate::routing::bundleManager;
use crate::routing::model::BundleKind;
use pathfinding::directed::dijkstra::dijkstra;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

// TODO : Add a get_node for the network layer

pub struct RoutingEngine {
    pub node_id: Uuid,
    pub graph: NetworkGraph,
    pub server: Server,
    pub bundle_manager: BundleManager,
}

impl RoutingEngine {
    pub fn new(node_id: Uuid) -> Self {
        RoutingEngine {
            node_id: node_id,
            graph: NetworkGraph::new(),
            server: Server::new(),
            bundle_manager: bundleManager::new(),
        }
    }

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
       pub async fn route_bundle(&self, bundle: &mut Bundle, retry_interval: Duration) {
           if matches!(bundle.kind, BundleKind::Ack { .. }) {
               if (bundle.source.id == self.node_id) {
                   bundle_manager.delete_bundle(bundle.id);
                   return;
               }

               bundle_manager.handle_incoming_ack(bundle);
               // Call the network layer
               let source = get_node(self.node_id);
               send_bundle(source, bundle);
               return;
           }

           //  Check if we are the destination
           if self.node_id == bundle.destination.id {
               bundle.shipment_status = super::model::MsgStatus::Delivered;
               let ack = Bundle::new_ack(bundle);
               bundle_manager.save_bundle(&ack);
               bundle_manager.delete_bundle(bundle.id);
               // Call the network layer
               // for peer in network.get_connected_peers() {
               //     network.send_bundle(peer, &ack);
               // }
               return;
           }

           // Check if TTL expired
           if bundle.is_expired() {
               bundle.shipment_status = super::model::MsgStatus::Expired;
               bundle_manager.delete_bundle(bundle.id);
               return;
           }

           // Find next hop if not we stay here
           let Some(next_hop) = self.find_next_hop(bundle.destination.id) else {
               bundle.shipment_status = super::model::MsgStatus::Pending;
               bundle_manager.save_bundle(bundle);
               self.forward_loop(bundle_manager, retry_interval).await;
               return;
           };

           // next hop found and we want to send to it missing bundles
           bundle.shipment_status = super::model::MsgStatus::InTransit;
           let local_sv = self.get_summary_vector(bundle_manager);
           // let peer_sv = network.get_peer_summary_vector(next_hop);
           // let to_forward = self.anti_entropy(&local_sv, &peer_sv);
           // call for network layer
           // for id in  to_forward {
           //     if let Some(b) = bundle_manager.get(id) {
           //         network.send_bundle(next_hop, &b)
           //     }
           // }
       }

}
