use super::*;
use std::collections::VecDeque;

impl Simulation {
    pub(super) fn shortest_active_path(
        &self,
        source: NodeId,
        destination: NodeId,
    ) -> Option<Vec<NodeId>> {
        if !self.graph.is_active(source) || !self.graph.is_active(destination) {
            return None;
        }
        let mut parent = vec![None; self.graph.node_count()];
        let mut queue = VecDeque::from([source]);
        parent[source as usize] = Some(source);
        while let Some(node) = queue.pop_front() {
            if node == destination {
                break;
            }
            for neighbor in self.graph.active_neighbors(node) {
                if parent[neighbor as usize].is_none() {
                    parent[neighbor as usize] = Some(node);
                    queue.push_back(neighbor);
                }
            }
        }
        parent[destination as usize]?;
        let mut path = vec![destination];
        while *path.last().unwrap() != source {
            path.push(parent[*path.last().unwrap() as usize]?);
        }
        path.reverse();
        Some(path)
    }
}
