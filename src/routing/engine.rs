use super::bundleManager::BundleManager; // this import provides storage-backed bundle lifecycle operations used by the router
use super::model::{Bundle, BundleKind, MsgStatus}; // this import provides bundle models and status enums used by routing logic
use crate::network::client::{request_peer_sv, send_bundle}; // this import provides tcp operations used for epidemic and anti-entropy forwarding
use crate::network::server::{PeerRecord, ServerRequest, ServerResponse}; // this import provides registry request and response models for connected peer lookup
use std::io::{Read, Write}; // this import provides stream io operations for registry synchronization
use std::net::TcpStream; // this import provides tcp connectivity to the registry service
use uuid::Uuid; // this import provides node and bundle identifiers for matching and filtering

#[derive(Debug, Clone)]
pub struct RoutingEngine {
    pub node_id: Uuid, // this stores the local node identifier that owns this routing engine
    pub peers: Vec<Uuid>, // this stores the configured peer ids that this node may exchange bundles with
    pub registry_addr: String, // this stores the registry endpoint used to discover currently connected peers
    pub bundle_manager: BundleManager, // this stores the persistent bundle manager for this node
}

impl RoutingEngine {
    pub fn new(node_id: Uuid, peers: Vec<Uuid>, name: String) -> Self {
        RoutingEngine {
            node_id,
            peers,
            registry_addr: "127.0.0.1:9100".to_string(), // this initializes default registry address so commands work without explicit override
            bundle_manager: BundleManager::new(node_id, name),
        }
    }

    pub fn get_summary_vector(&self, bundle_manager: &BundleManager) -> Vec<Bundle> {
        bundle_manager.get_bundles_from_node() // this returns local inventory used by anti-entropy diffing
    }

    pub fn anti_entropy<'a>(&self, local_sv: &'a [Bundle], peer_sv: &[Uuid]) -> Vec<&'a Bundle> {
        let mut missing_on_peer: Vec<&Bundle> = vec![]; // this collects local bundles that the remote peer does not have yet
        for bundle in local_sv {
            if !peer_sv.contains(&bundle.id) {
                missing_on_peer.push(bundle); // this schedules a bundle for forwarding when remote summary vector misses it
            }
        }
        missing_on_peer
    }

    pub fn get_peer_summary_vector(&self, peer_addr: &str, peer_port: u16) -> Vec<Uuid> {
        let destination_address = format!("{}:{}", peer_addr, peer_port); // this builds tcp endpoint used for summary-vector control query
        match request_peer_sv(self.node_id, destination_address) {
            Ok(ids) => ids, // this returns peer inventory ids for anti-entropy diff
            Err(e) => {
                eprintln!(
                    "[{}] could not get summary vector from {}:{}: {}",
                    self.node_id, peer_addr, peer_port, e
                ); // this logs peer query failures without crashing routing loop
                vec![] // this falls back to empty peer vector which means no forwarding for that peer in this round
            }
        }
    }

    pub async fn route_bundle(&mut self, bundle: &mut Bundle) {
        let already_known = self.bundle_manager.has_bundle(bundle.id); // this checks dedup state before storing and forwarding to prevent routing loops
        if !already_known {
            let _ = self.bundle_manager.save_bundle(bundle); // this persists unseen bundles before any routing action for crash-safe forwarding
        } else {
            return; // this skips already-known bundles to avoid repeated anti-entropy fanout of duplicates
        }

        let mut connected_peers: Vec<PeerRecord> = Vec::new(); // this collects currently connected neighbors from the registry for synchronized forwarding
        if let Ok(mut stream) = TcpStream::connect(&self.registry_addr) {
            let request = ServerRequest::GetConnectedPeers(self.peers.clone()); // this asks the registry for active peers among the node's configured neighbor list
            if let Ok(payload) = serde_json::to_vec(&request) {
                if stream.write_all(&payload).is_ok() {
                    let mut buffer = [0u8; 16384]; // this buffer captures one registry response payload
                    if let Ok(n) = stream.read(&mut buffer) {
                        if n > 0 {
                            if let Ok(ServerResponse::Peers(peers)) =
                                serde_json::from_slice::<ServerResponse>(&buffer[..n])
                            {
                                connected_peers = peers
                                    .into_iter()
                                    .filter(|peer| peer.node.id != self.node_id)
                                    .collect(); // this removes self-records so routing never sends to its own listener
                            }
                        }
                    }
                }
            }
        }

        if bundle.is_expired() {
            let _ = self.bundle_manager.update_status(bundle.id, MsgStatus::Expired); // this marks the bundle as expired before removal to keep storage lifecycle explicit
            let _ = self.bundle_manager.delete_bundle(bundle.id); // this removes expired bundles so anti-entropy does not propagate stale data
            return;
        }

        if matches!(&bundle.kind, BundleKind::Ack { .. }) {
            if self.node_id == bundle.destination.id {
                if let BundleKind::Ack { ack_bundle_id } = &bundle.kind {
                    let _ = self.bundle_manager.delete_bundle(*ack_bundle_id); // this deletes original data at source once ack arrives at final destination
                }
                let _ = self.bundle_manager.update_status(bundle.id, MsgStatus::Delivered); // this marks the ack as delivered at its target node
                let _ = self.bundle_manager.delete_bundle(bundle.id); // this removes completed ack bundles from storage after final delivery
                return;
            }
        }

        if self.node_id == bundle.destination.id {
            let _ = self.bundle_manager.update_status(bundle.id, MsgStatus::Delivered); // this marks inbound data as delivered when local node is final destination
            let ack = Bundle::new_ack(bundle); // this creates a reverse acknowledgment bundle for source-side cleanup
            let _ = self.bundle_manager.save_bundle(&ack); // this persists the ack before forwarding to avoid losing it on process restart
            for connected_peer in &connected_peers {
                let destination_address = format!(
                    "{}:{}",
                    connected_peer.node.address, connected_peer.node.port
                ); // this computes transport endpoint for ack propagation
                send_bundle(self.node_id, &ack, destination_address); // this forwards the ack to all currently connected peers for epidemic return path
            }
            return;
        }

        let local_sv = self.get_summary_vector(&self.bundle_manager); // this captures local inventory snapshot before anti-entropy synchronization
        let forwardable_bundles: Vec<Bundle> = local_sv
            .into_iter()
            .filter(|candidate| {
                matches!(
                    candidate.shipment_status,
                    MsgStatus::Pending | MsgStatus::InTransit
                )
            })
            .collect(); // this keeps only active bundles that are eligible for forwarding

        for connected_peer in connected_peers {
            let peer_sv = self.get_peer_summary_vector(
                connected_peer.node.address.as_str(),
                connected_peer.node.port,
            ); // this fetches remote inventory so anti-entropy forwards only missing bundles

            let to_forward = self.anti_entropy(&forwardable_bundles, &peer_sv); // this computes the per-peer bundle delta to synchronize state without duplicates
            let destination_address =
                format!("{}:{}", connected_peer.node.address, connected_peer.node.port); // this computes socket endpoint for bundle forwarding

            for candidate in to_forward {
                send_bundle(self.node_id, candidate, destination_address.clone()); // this forwards each missing bundle to the connected peer
                let _ = self
                    .bundle_manager
                    .update_status(candidate.id, MsgStatus::InTransit); // this updates persistent status after handoff attempt so layers stay synchronized
            }
        }
    }
}
