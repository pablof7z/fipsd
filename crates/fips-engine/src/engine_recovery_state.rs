use super::*;

impl GraphRecoveryRuntime {
    pub(in crate::engine) fn has_session(&self, source: NodeId, destination: NodeId) -> bool {
        self.sessions.contains_key(&(source, destination))
    }

    pub(in crate::engine) fn session_snapshot(&self) -> Vec<(NodeId, NodeId, Vec<NodeId>)> {
        self.sessions
            .iter()
            .map(|((source, destination), path)| (*source, *destination, path.clone()))
            .collect()
    }

    pub(in crate::engine) fn insert_session(
        &mut self,
        source: NodeId,
        destination: NodeId,
        path: Vec<NodeId>,
    ) {
        self.sessions.insert((source, destination), path);
    }

    pub(in crate::engine) fn remove_session(
        &mut self,
        source: NodeId,
        destination: NodeId,
    ) -> bool {
        let removed = self.sessions.remove(&(source, destination)).is_some();
        if removed {
            self.resources[source as usize].release(ResourceKind::Sessions, 1);
        }
        removed
    }

    pub(in crate::engine) fn invalidate_all_caches(&mut self) -> u64 {
        self.caches
            .iter_mut()
            .map(CoordinateCache::invalidate_all)
            .sum()
    }

    pub(in crate::engine) fn disrupt_sessions_for_node(&mut self, node: NodeId) -> u64 {
        let disrupted = self
            .sessions
            .keys()
            .copied()
            .filter(|(source, destination)| *source == node || *destination == node)
            .collect::<Vec<_>>();
        for (source, destination) in &disrupted {
            self.sessions.remove(&(*source, *destination));
            self.resources[*source as usize].release(ResourceKind::Sessions, 1);
        }
        self.counters.session_disruptions += disrupted.len() as u64;
        disrupted.len() as u64
    }

    pub(in crate::engine) fn disrupt_sessions_for_edge(
        &mut self,
        left: NodeId,
        right: NodeId,
    ) -> u64 {
        let disrupted = self
            .sessions
            .iter()
            .filter(|(_, path)| {
                path.windows(2).any(|pair| {
                    (pair[0] == left && pair[1] == right) || (pair[0] == right && pair[1] == left)
                })
            })
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for (source, destination) in &disrupted {
            self.sessions.remove(&(*source, *destination));
            self.resources[*source as usize].release(ResourceKind::Sessions, 1);
        }
        self.counters.session_disruptions += disrupted.len() as u64;
        disrupted.len() as u64
    }
}
