use super::bundleManager::BundleManager;
use super::epidemic::NetworkGraph;
use super::model::{Bundle, Node};
use crate::network::client::send_bundle;
use crate::network::server::Server;
use crate::routing::model::BundleKind;
use std::time::Duration;
use uuid::Uuid;

pub struct RoutingEngine {
    pub current_node: Node,     // je recupere tout les infos de na node 
    pub graph: NetworkGraph,
    pub server: Server,
    pub bundle_manager: BundleManager,
}

impl RoutingEngine {
    pub fn new(current_node: Node) -> Self {
        let node_id = current_node.id;
        RoutingEngine {
            current_node,
            graph: NetworkGraph::new(),
            server: Server::new(),
            bundle_manager: BundleManager::new(node_id),
        }
    }

    // Summary vector management
    pub fn get_summary_vector(&self, bundle_manager: &BundleManager) -> Vec<Uuid> {
        return bundle_manager.get_bundles_from_node(self.current_node.id); // this function calls the storage layer to get the bundles stored
    }

    pub fn anti_entropy(&self, local_sv: &[Uuid], peer_sv: &[Uuid]) -> Vec<Uuid> {
        let mut missing_on_peer: Vec<Uuid> = vec![];
        for &i in local_sv.iter() {
            if !peer_sv.contains(&i) {
                missing_on_peer.push(i);
            }
        }
        missing_on_peer
    }

    // Epidemic propagation -- on recupere les voisisons 
    pub fn get_neighbors_for_epidemic(&self) -> Vec<Uuid> {
        self.graph.neighbors(&self.current_node.id)
            .into_iter()
            .map(|(neighbor_id, _)| neighbor_id)
            .collect()
    }

    /// epedimic flooding avec antientropy 
    /// envoie le bundle a tous les voisins via send_bundle (qui diffuse a tous les pairs connectes)
    /// utilise l'antientropy pour minimiser les transmissions en double
    
    fn propagate_via_epidemic(&mut self, bundle: &mut Bundle) {
        let neighbors = self.get_neighbors_for_epidemic();

        if neighbors.is_empty() {
            // pas de voisins disponibles stocker le bundle localement en état Pending pour une tentative ultérieure
            bundle.shipment_status = super::model::MsgStatus::Pending;
            self.bundle_manager.save_bundle(bundle);
            return;
        }

        let _local_sv = self.get_summary_vector(&self.bundle_manager);

        // on marque le bundle comme en transit et on le stocke localement pour pouvoir le repropager plus tard si besoin (en cas de nouveaux voisins ou de retransmission)
        bundle.shipment_status = super::model::MsgStatus::InTransit;
        self.bundle_manager.save_bundle(bundle);

        // epidemic follding va envoyer le bundle a tout les voisisn
        // j'ai decider de mettre le current node comme source du bundle pour que les voisins puissent savoir que c'est nous qui envoyons le bundle et pas le node original 

        send_bundle(&self.current_node, bundle);
    }

    // Main routing decision with epidemic flooding logic
    pub async fn route_bundle(&mut self, bundle: &mut Bundle, _retry_interval: Duration) {
        // ACK
        if matches!(bundle.kind, BundleKind::Ack { .. }) {
            if bundle.source.id == self.current_node.id {
                // ACK reached the original sender - cleanup
                self.bundle_manager.delete_bundle(bundle.id);
                return;
            }

            self.bundle_manager.handle_incoming_ack(bundle);
            
        
            self.propagate_via_epidemic(bundle);
            return;
        }

        
        if self.current_node.id == bundle.destination.id {
            bundle.shipment_status = super::model::MsgStatus::Delivered;
            
            
            let mut ack = Bundle::new_ack(bundle);
            self.bundle_manager.save_bundle(&ack);
            self.bundle_manager.delete_bundle(bundle.id);
            
            
            // propager ack back vers la source avec epedimic flooding
            self.propagate_via_epidemic(&mut ack);
            return;
        }

        // 3. Check if TTL expired
        if bundle.is_expired() {
            bundle.shipment_status = super::model::MsgStatus::Expired;
            self.bundle_manager.delete_bundle(bundle.id);
            return;
        }

        // epidemic flooding pour tout les voisins avec la deduplication
        self.propagate_via_epidemic(bundle);
    }
}
