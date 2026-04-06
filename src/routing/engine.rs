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
    pub registry_address: String,
    pub server: Server,
    pub bundle_manager: BundleManager,
}

impl RoutingEngine {
    pub fn new(node_id: Uuid, peers: Vec<Uuid>, name: String) -> Self {
        RoutingEngine {
            node_id,
            peers,
            registry_address: "127.0.0.1:9100".to_string(),
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
            let mut requested_ids = self.peers.clone(); 
            if !requested_ids.contains(&bundle.destination.id) {
                requested_ids.push(bundle.destination.id); // force-include ACK destination so backward route can reach original sender
            }

            let connected_peers: Vec<_> =
                get_connected_peers_from_server(&self.registry_address, &requested_ids)
                .into_iter()
                .filter(|p| p.node.id != self.node_id)
                .collect();

            if self.node_id == bundle.destination.id {
                self.bundle_manager.handle_incoming_ack(bundle);

                bundle.shipment_status = super::model::MsgStatus::Delivered;
                self.bundle_manager.upsert_bundle(bundle);
                return;
            }

            // Forward only first-seen ACKs to limit duplicate loops.
            if !self.bundle_manager.handle_incoming_ack(bundle) {
                return;
            }

            let mut peers_needing_ack = Vec::new();
            for peer in connected_peers {
                let peer_sv = self.get_peer_summary_vector(
                    peer.node.address.as_str(),
                    peer.node.port,
                );
                if !peer_sv.contains(&bundle.id) {
                    peers_needing_ack.push(peer);
                }
            }

            let mut sent_count = 0usize;
            for peer in peers_needing_ack {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                if send_bundle(self.node_id, bundle, destination_adress) {
                    sent_count += 1;
                }
            }

            if sent_count > 0 {
                bundle.shipment_status = super::model::MsgStatus::InTransit;
                self.bundle_manager.upsert_bundle(bundle);
            }
            return;
        }

        // Check if TTL expired
        if bundle.is_expired() {
            bundle.shipment_status = super::model::MsgStatus::Expired;
            self.bundle_manager.delete_bundle(bundle.id);
            return;
        }

        let mut ack_requested_ids = self.peers.clone();
        if !ack_requested_ids.contains(&bundle.source.id) {
            ack_requested_ids.push(bundle.source.id);
        }

        let connected_peers: Vec<_> =
            get_connected_peers_from_server(&self.registry_address, &ack_requested_ids)
            .into_iter()
            .filter(|p| p.node.id != self.node_id)
            .collect();

        // If we are the destination, keep the data bundle as delivered and propagate the ACK.
        if self.node_id == bundle.destination.id {
            bundle.shipment_status = super::model::MsgStatus::Delivered;
            self.bundle_manager.upsert_bundle(bundle);

            let ack = Bundle::new_ack(bundle);

            // ACK id is deterministic per DATA bundle id; store and forward only once.
            let created_here = if self.bundle_manager.has_bundle(ack.id) {
                false
            } else {
                self.bundle_manager.save_bundle(&ack)
            };

            if !created_here {
                return;
            }

            let mut peers_needing_ack = Vec::new();
            for peer in connected_peers {
                let peer_sv = self.get_peer_summary_vector(
                    peer.node.address.as_str(),
                    peer.node.port,
                );
                if !peer_sv.contains(&ack.id) {
                    peers_needing_ack.push(peer);
                }
            }

            let mut sent_count = 0usize;
            for peer in peers_needing_ack {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                if send_bundle(self.node_id, &ack, destination_adress) {
                    sent_count += 1;
                }
            }

            if sent_count > 0 {
                let mut ack_in_transit = ack.clone();
                ack_in_transit.shipment_status = super::model::MsgStatus::InTransit;
                self.bundle_manager.upsert_bundle(&ack_in_transit);
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

    pub fn retry_unsynced_bundles(&mut self) {
        let connected_peers: Vec<_> = get_connected_peers_from_server(&self.peers)
            .into_iter()
            .filter(|p| p.node.id != self.node_id)
            .collect();

        if connected_peers.is_empty() {
            return;
        }

        let local_sv = Self::get_summary_vector(&mut self.bundle_manager);
        let retryable_bundles: Vec<Bundle> = local_sv
            .into_iter()
            .filter(|b| {
                b.shipment_status == super::model::MsgStatus::Pending
                    || b.shipment_status == super::model::MsgStatus::InTransit
            })
            .collect();

        if retryable_bundles.is_empty() {
            return;
        }

        let mut sent_bundle_ids: Vec<Uuid> = Vec::new();
        for connected_peer in connected_peers {
            let peer_sv = self.get_peer_summary_vector(
                connected_peer.node.address.as_str(),
                connected_peer.node.port,
            );

            let to_forward = self.anti_entropy(&retryable_bundles, &peer_sv);
            let destination_adress = format!(
                "{}:{}",
                connected_peer.node.address, connected_peer.node.port
            );

            for bundle in to_forward {
                if send_bundle(self.node_id, bundle, destination_adress.clone())
                    && !sent_bundle_ids.contains(&bundle.id)
                {
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
