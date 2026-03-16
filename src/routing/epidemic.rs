// We will implement the controlled epidemic routing usign djikstra
// the link to the research paper used as a reference is : https://rjwave.org/ijedr/papers/IJEDR1603115.pdf
use std::collections::HashMap;
use uuid::Uuid;

pub struct NetworkGraph {
    adjacency: HashMap<Uuid, Vec<(Uuid, u32)>>,
}

impl NetworkGraph {
    pub fn neighbors(&self, node: &Uuid) -> Vec<(Uuid, u32)> {
        self.adjacency.get(node).cloned().unwrap_or_default()
    }
}
