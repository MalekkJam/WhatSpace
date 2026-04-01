use super::bundleManager::BundleManager;
use super::model::Bundle;
use crate::network::client::{request_peer_sv, send_bundle};
use crate::network::server::{PeerRecord, Server, ServerRequest, ServerResponse};
use crate::routing::model::BundleKind;
use std::io::{Read, Write};
use std::net::TcpStream;
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

    /// Interroge le vrai serveur de registre (127.0.0.1:8080) pour obtenir
    /// la liste des nœuds actuellement connectés, au lieu du serveur local vide.
    fn query_registry_peers(&self) -> Vec<PeerRecord> {
        let mut stream = match TcpStream::connect("127.0.0.1:8080") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[{}] cannot reach registry: {}", self.node_id, e);
                return vec![];
            }
        };
        let req = ServerRequest::GetConnectedPeers(vec![]);
        let msg = match serde_json::to_string(&req) {
            Ok(m) => m,
            Err(_) => return vec![],
        };
        if stream.write_all(msg.as_bytes()).is_err() {
            return vec![];
        }
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .ok();
        let mut buf = [0u8; 4096];
        match stream.read(&mut buf) {
            Ok(n) if n > 0 => match serde_json::from_slice::<ServerResponse>(&buf[..n]) {
                Ok(ServerResponse::Peers(peers)) => peers,
                _ => vec![],
            },
            _ => vec![],
        }
    }

    pub fn process_received_bundle(node_id: Uuid, bundle: &mut Bundle, bundle_manager: &mut BundleManager) {
        
        eprintln!("[{}] Processing received bundle {} (dest: {})", 
            node_id, bundle.id, bundle.destination.id);
        
        // Check if TTL expired first
        if bundle.is_expired() {
            eprintln!("[{}] Bundle {} expired, discarding", node_id, bundle.id);
            bundle.shipment_status = super::model::MsgStatus::Expired;
            return;
        }
        
        // Check if we are the destination
        if node_id == bundle.destination.id {
            eprintln!("[{}] I am destination for bundle {}, marking as Delivered", node_id, bundle.id);
            bundle.shipment_status = super::model::MsgStatus::Delivered;
            if !bundle_manager.save_bundle(bundle) {
                let _ = bundle_manager.update_bundle(bundle);
            }
            return;
        }
        
        // Otherwise, we're a relay node and the bundle is in transit.
        bundle.shipment_status = super::model::MsgStatus::InTransit;
        if !bundle_manager.save_bundle(bundle) {
            let _ = bundle_manager.update_bundle(bundle);
        }
        eprintln!("[{}] Saving bundle {} as relay node", node_id, bundle.id);
    }

    pub async fn route_bundle(&mut self, bundle: &mut Bundle) {
        self.bundle_manager.save_bundle(bundle);

        if matches!(bundle.kind, BundleKind::Ack { .. }) {
            if bundle.source.id == self.node_id {
                self.bundle_manager.delete_bundle(bundle.id);
                return;
            }

            self.bundle_manager.handle_incoming_ack(bundle);

            for peer in self.query_registry_peers() {
                let destination_adress = format!("{}:{}", peer.node.address, peer.node.port);
                send_bundle(peer.node.id, bundle, destination_adress);
            }
            return;
        }

        //  Check if we are the destination
        if self.node_id == bundle.destination.id {
            bundle.shipment_status = super::model::MsgStatus::Delivered;
            if !self.bundle_manager.save_bundle(bundle) {
                let _ = self.bundle_manager.update_bundle(bundle);
            }
            return;
        }

        // Check if TTL expired
        if bundle.is_expired() {
            bundle.shipment_status = super::model::MsgStatus::Expired;
            let _ = self.bundle_manager.update_bundle(bundle);
            return;
        }

        // propagate to all my peers
        let local_sv = self.get_summary_vector(&self.bundle_manager);
        let pending_bundles: Vec<Bundle> = local_sv
            .into_iter()
            .filter(|b| b.shipment_status == super::model::MsgStatus::Pending)
            .collect();
        let connected_peers = self.query_registry_peers();

        for connected_peer in connected_peers {
            let peer_sv = self.get_peer_summary_vector(
                connected_peer.node.address.as_str(),
                connected_peer.node.port,
            );
            // Compare against what the peer already has
            let to_forward = self.anti_entropy(&pending_bundles, &peer_sv);
            let destination_adress: String = format!(
                "{}:{}",
                connected_peer.node.address, connected_peer.node.port
            );
            for bundle in to_forward {
                send_bundle(self.node_id, bundle, destination_adress.clone());
            }
        }

        bundle.shipment_status = super::model::MsgStatus::InTransit;
        let _ = self.bundle_manager.update_bundle(bundle);
    }
}
