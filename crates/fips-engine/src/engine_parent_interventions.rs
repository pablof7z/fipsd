use super::*;

impl Simulation {
    pub(super) fn handle_parent_cost(
        &mut self,
        event: EventId,
        evidence: &str,
        input: ParentCostInput,
    ) -> Result<Value, RunError> {
        let cause = format!("input:{}", input.id);
        self.add_ledger(&cause, "requested", 1, evidence);
        let node = match input.target {
            Some(node) => node,
            None => self
                .graph
                .node_ids()
                .find(|node| self.parent_pair(*node).is_some())
                .ok_or_else(|| {
                    RunError::Unsupported(format!(
                        "{} found no converged node with two eligible parents",
                        input.action
                    ))
                })?,
        };
        let (old_parent, alternate) = self.parent_pair(node).ok_or_else(|| {
            RunError::Unsupported(format!(
                "{} requires node {node} to have a current and alternate same-root parent",
                input.action
            ))
        })?;
        let old_ancestry = self.graph.ancestry(node).to_vec();
        let old_edge = self.graph.edge_between(node, old_parent).ok_or_else(|| {
            RunError::Invariant(format!(
                "current parent {old_parent} is not adjacent to {node}"
            ))
        })?;
        let alternate_edge = self.graph.edge_between(node, alternate).ok_or_else(|| {
            RunError::Invariant(format!(
                "alternate parent {alternate} is not adjacent to {node}"
            ))
        })?;
        let eligible = self.eligible_parent_peers(node);
        let before = eligible
            .iter()
            .map(|peer| {
                let edge = self.graph.edge_between(node, *peer).unwrap();
                json!({
                    "peer": peer,
                    "edge": edge,
                    "cost_ppm": self.parent_cost_ppm[edge as usize]
                })
            })
            .collect::<Vec<_>>();
        for peer in eligible {
            let edge = self.graph.edge_between(node, peer).unwrap();
            self.parent_cost_ppm[edge as usize] = if peer == alternate {
                input.preferred_cost_ppm
            } else {
                input.degraded_cost_ppm
            };
        }
        let transition = self.evaluate_parent(node)?;
        let switched = transition.is_some();
        if let Some(transition) = transition {
            self.apply_transition(node, transition, &cause, Some(event), evidence)?;
        } else {
            self.add_ledger(&cause, "suppressed", 1, evidence);
        }
        self.add_ledger(&cause, "mmp-cost-samples", 2, evidence);
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({
            "intervention_id": input.id,
            "action": input.action,
            "phase": input.phase,
            "node": node,
            "old_parent": old_parent,
            "preferred_parent": alternate,
            "new_parent": self.graph.parent(node),
            "switched": switched,
            "suppressed": !switched,
            "old_ancestry": old_ancestry,
            "new_ancestry": self.graph.ancestry(node),
            "old_parent_edge": old_edge,
            "preferred_parent_edge": alternate_edge,
            "preferred_cost_ppm": input.preferred_cost_ppm,
            "degraded_cost_ppm": input.degraded_cost_ppm,
            "cost_source": "modeled-mmp-fixed-point",
            "before": before
        }))
    }

    fn parent_pair(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        if !self.graph.is_active(node) {
            return None;
        }
        let current = self.graph.parent(node)?;
        let current_snapshot = self.peer_views.get(&(node, current))?;
        let alternate = self
            .eligible_parent_peers(node)
            .into_iter()
            .filter(|peer| *peer != current)
            .filter(|peer| {
                self.peer_views
                    .get(&(node, *peer))
                    .is_some_and(|snapshot| snapshot.root == current_snapshot.root)
            })
            .min_by_key(|peer| (self.graph.address(*peer).ok(), *peer))?;
        Some((current, alternate))
    }

    fn eligible_parent_peers(&self, node: NodeId) -> Vec<NodeId> {
        self.graph
            .active_neighbors(node)
            .into_iter()
            .filter(|peer| {
                self.peer_views
                    .get(&(node, *peer))
                    .is_some_and(|snapshot| !snapshot.ancestry.contains(&node))
            })
            .collect()
    }
}
