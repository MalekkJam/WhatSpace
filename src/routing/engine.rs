use super::bundleManager::BundleManager;
use super::model::Bundle;
use crate::network::client::{get_connected_peers_from_server, request_peer_sv, send_bundle};
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
    pub fn get_summary_vector(bundle_manager: &mut BundleManager) -> Vec<Bundle> {
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
        if matches!(bundle.kind, BundleKind::Ack { .. }) {
            let connected_peers: Vec<_> = get_connected_peers_from_server(&self.peers)
                .into_iter()
                .filter(|p| p.node.id != self.node_id)
                .collect();

            if self.node_id == bundle.destination.id {
                if let BundleKind::Ack { ack_bundle_id } = &bundle.kind {
                    if let Some(mut delivered_bundle) = self.bundle_manager.get(*ack_bundle_id) {
                        delivered_bundle.shipment_status = super::model::MsgStatus::Delivered;
                        self.bundle_manager.upsert_bundle(&delivered_bundle);
                    }
                }

                bundle.shipment_status = super::model::MsgStatus::Delivered;
                self.bundle_manager.upsert_bundle(bundle);
                return;
            }

            self.bundle_manager.handle_incoming_ack(bundle);

            for peer in connected_peers {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                send_bundle(peer.node.id, bundle, destination_adress);
            }
            return;
        }

        // Check if TTL expired
        if bundle.is_expired() {
            bundle.shipment_status = super::model::MsgStatus::Expired;
            self.bundle_manager.delete_bundle(bundle.id);
            return;
        }

        let connected_peers: Vec<_> = get_connected_peers_from_server(&self.peers)
            .into_iter()
            .filter(|p| p.node.id != self.node_id)
            .collect();

        // If we are the destination, keep the data bundle as delivered and propagate the ACK.
        if self.node_id == bundle.destination.id {
            bundle.shipment_status = super::model::MsgStatus::Delivered;
            self.bundle_manager.upsert_bundle(bundle);

            let ack = Bundle::new_ack(bundle);
            self.bundle_manager.save_bundle(&ack);
            for peer in connected_peers {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                send_bundle(peer.node.id, &ack, destination_adress);
            }
            return;
        }

        self.bundle_manager.save_bundle(bundle);
        let local_sv = Self::get_summary_vector(&mut self.bundle_manager);

        let pending_bundles: Vec<Bundle> = local_sv
            .into_iter()
            .filter(|b| b.shipment_status == super::model::MsgStatus::Pending)
            .collect();

        let mut sent_to_peer = false;
        for connected_peer in connected_peers {
            let peer_sv = self.get_peer_summary_vector(
                connected_peer.node.address.as_str(),
                connected_peer.node.port,
            );

            // Then compare against what the peer already has
            let to_forward = self.anti_entropy(&pending_bundles, &peer_sv);

            let destination_adress: String = format!(
                "{}:{}",
                connected_peer.node.address, connected_peer.node.port
            );

            for bundle in to_forward {
                send_bundle(self.node_id, bundle, destination_adress.clone());
                sent_to_peer = true;
            }
        }

        if sent_to_peer {
            bundle.shipment_status = super::model::MsgStatus::InTransit;
            self.bundle_manager.upsert_bundle(bundle);
        }
    }

    pub fn retry_pending_bundles(&mut self) {
        let connected_peers: Vec<_> = get_connected_peers_from_server(&self.peers)
            .into_iter()
            .filter(|p| p.node.id != self.node_id)
            .collect();

        if connected_peers.is_empty() {
            return;
        }

        let local_sv = Self::get_summary_vector(&mut self.bundle_manager);
        let pending_bundles: Vec<Bundle> = local_sv
            .into_iter()
            .filter(|b| {
                b.shipment_status == super::model::MsgStatus::Pending
                    && matches!(b.kind, BundleKind::Data { .. })
            })
            .collect();

        if pending_bundles.is_empty() {
            return;
        }

        let mut sent_bundle_ids: Vec<Uuid> = Vec::new();
        for connected_peer in connected_peers {
            let peer_sv = self.get_peer_summary_vector(
                connected_peer.node.address.as_str(),
                connected_peer.node.port,
            );

            let to_forward = self.anti_entropy(&pending_bundles, &peer_sv);
            let destination_adress = format!(
                "{}:{}",
                connected_peer.node.address, connected_peer.node.port
            );

            for bundle in to_forward {
                send_bundle(self.node_id, bundle, destination_adress.clone());
                if !sent_bundle_ids.contains(&bundle.id) {
                    sent_bundle_ids.push(bundle.id);
                }
            }
        }

        for bundle_id in sent_bundle_ids {
            if let Some(mut bundle) = self.bundle_manager.get(bundle_id) {
                bundle.shipment_status = super::model::MsgStatus::InTransit;
                self.bundle_manager.upsert_bundle(&bundle);
            }
        }
    }
}
