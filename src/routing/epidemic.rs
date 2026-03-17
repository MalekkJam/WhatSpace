// We will implement the controlled epidemic routing usign djikstra
// the link to the research paper used as a reference is : https://rjwave.org/ijedr/papers/IJEDR1603115.pdf
use std::collections::HashMap;
use uuid::Uuid;

pub struct NetworkGraph {
    pub adjacency: HashMap<Uuid, Vec<(Uuid, u32)>>,
}

impl NetworkGraph {
    pub fn new() -> Self {
        NetworkGraph {
            adjacency: HashMap::new(),
        }
    }

    pub fn new_from_adjacency(adjacency: HashMap<Uuid, Vec<(Uuid, u32)>>) -> Self {
        NetworkGraph { adjacency }
    }

    pub fn add_edge(&mut self, source: Uuid, destination: Uuid, cost: u32) {
        self.adjacency
            .entry(source)
            .or_insert_with(Vec::new)
            .push((destination, cost));
    }

    pub fn neighbors(&self, node: &Uuid) -> Vec<(Uuid, u32)> {
        self.adjacency.get(node).cloned().unwrap_or_default()
    }
}
