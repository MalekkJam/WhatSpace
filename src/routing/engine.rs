use super::bundleManager::BundleManager;
use super::model::Bundle;
use crate::network::client::{request_peer_sv, send_bundle};
use crate::network::server::Server;
use crate::routing::model::BundleKind;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RoutingEngine {
    pub node_id: Uuid,
    pub peers: Vec<Uuid>,
    pub server: Server,
    pub bundle_manager: BundleManager,
}

impl RoutingEngine {
    pub fn new(node_id: Uuid, peers: Vec<Uuid>, name: String) -> Self {
        RoutingEngine {
            node_id,
            peers,
            server: Server::new(),
            bundle_manager: BundleManager::new(node_id, name),
        }
    }

    // Summary vector management
    pub fn get_summary_vector(&self, bundle_manager: &BundleManager) -> Vec<Bundle> {
        return bundle_manager.get_bundles_from_node(); // this function calls the storage layer to get the bundles stored
    }

    pub fn anti_entropy<'a>(&self, local_sv: &'a [Bundle], peer_sv: &[Uuid]) -> Vec<&'a Bundle> {
        let mut missing_on_peer: Vec<&'a Bundle> = vec![];
        for i in local_sv.iter() {
            if !peer_sv.contains(&i.id) {
                missing_on_peer.push(i);
            }
        }
        missing_on_peer
    }

    pub fn get_peer_summary_vector(&self, peer_addr: &str, peer_port: u16) -> Vec<Uuid> {
        let destination_adress = format!("{}:{}", peer_addr.to_string(), peer_port);
        match request_peer_sv(self.node_id, destination_adress) {
            Ok(ids) => ids,
            Err(e) => {
                eprintln!(
                    "[{}] could not get SV from {}: {}",
                    self.node_id, peer_addr, e
                );
                vec![]
            }
        }
    }

    pub async fn route_bundle(&mut self, bundle: &mut Bundle) {
        self.bundle_manager.save_bundle(bundle);

        if matches!(bundle.kind, BundleKind::Ack { .. }) {
            if bundle.source.id == self.node_id {
                self.bundle_manager.delete_bundle(bundle.id);
                return;
            }

            self.bundle_manager.handle_incoming_ack(bundle);

            for peer in self.server.get_connected_peers(&self.peers) {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                send_bundle(peer.node.id, bundle, destination_adress);
            }
            return;
        }

        //  Check if we are the destination
        if self.node_id == bundle.destination.id {
            bundle.shipment_status = super::model::MsgStatus::Delivered;
            let ack = Bundle::new_ack(bundle);
            self.bundle_manager.save_bundle(&ack);
            self.bundle_manager.delete_bundle(bundle.id);
            for peer in self.server.get_connected_peers(&self.peers) {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                send_bundle(peer.node.id, &ack, destination_adress);
            }
            return;
        }

        // Check if TTL expired
        if bundle.is_expired() {
            bundle.shipment_status = super::model::MsgStatus::Expired;
            self.bundle_manager.delete_bundle(bundle.id);
            return;
        }

        // propagate to all my peers
        eprintln!("WE HEREEEEEE");
        let local_sv = self.get_summary_vector(&self.bundle_manager);
        eprintln!("WE HEREEEEEE");
        let pending_bundles: Vec<Bundle> = local_sv
            .into_iter()
            .filter(|b| b.shipment_status == super::model::MsgStatus::Pending)
            .collect();
        let connected_peers = self.server.get_connected_peers(&self.peers);
        
        eprintln!("WE HEREEEEEE");

        for connected_peer in connected_peers {
            let peer_sv = self.get_peer_summary_vector(
                connected_peer.node.address.as_str(),
                connected_peer.node.port,
            );
            eprintln!("WE HEREEEEEE");
            // Then compare against what the peer already has
            let to_forward = self.anti_entropy(&pending_bundles, &peer_sv);
            eprintln!("WE HEREEEEEE");
            let destination_adress: String = format!(
                "{}:{}",
                connected_peer.node.address, connected_peer.node.port
            );
            eprintln!("WE HEREEEEEE");
            for bundle in to_forward {
                send_bundle(self.node_id, bundle, destination_adress.clone());
            }
        }
        eprintln!("WE HEREEEEEE");
        bundle.shipment_status = super::model::MsgStatus::InTransit;
    }
}
