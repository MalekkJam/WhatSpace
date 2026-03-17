mod routing;
mod storage;

use routing::{RoutingEngine, NetworkGraph};
use routing::model::{Node, Bundle, BundleKind};

fn main() {
    // Create two nodes
    let node_a = Node::new("192.168.1.1", 5000, vec![]);
    let node_b = Node::new("192.168.1.2", 5001, vec![]);

    // Create network graph
    let mut graph = NetworkGraph::new();
    graph.add_edge(node_a.id, node_b.id, 1);
    graph.add_edge(node_b.id, node_a.id, 1);

    // Create routing engine
    let engine = RoutingEngine {
        node_id: node_a.id,
        graph,
    };

    // Create a bundle (message)
    let bundle = Bundle::new(
        node_a.clone(),
        node_b.clone(),
        BundleKind::Data {
            msg: "Hello from Node A".to_string(),
        },
        3600,
    );

    // Test anti-entropy
    use uuid::Uuid;
    let bundle_uuid = Uuid::new_v4();
    let local_bundles = vec![bundle_uuid];
    let peer_bundles = vec![];
    let to_sync = engine.anti_entropy(&local_bundles, &peer_bundles);

    println!("Routing Engine Demo");
    println!("==================");
    println!("Network: {} <-> {}", node_a.address, node_b.address);
    println!("Bundle: {} -> {}", bundle.source.address, bundle.destination.address);
    println!("Anti-entropy: {} bundles to sync", to_sync.len());
}
