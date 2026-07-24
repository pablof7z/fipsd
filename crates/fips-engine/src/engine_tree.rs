use super::*;

impl Simulation {
    pub(super) fn evaluate_parent(&self, node: NodeId) -> Result<Option<Transition>, RunError> {
        let own_address = self.graph.address(node)?;
        let mut candidates = self
            .graph
            .active_neighbors(node)
            .into_iter()
            .filter_map(|peer| {
                self.peer_views
                    .get(&(node, peer))
                    .filter(|snapshot| !snapshot.ancestry.contains(&node))
                    .and_then(|snapshot| {
                        self.parent_effective_depth_ppm(node, peer, snapshot)
                            .ok()
                            .map(|cost| (peer, snapshot, cost))
                    })
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(None);
        }
        candidates.sort_by_key(|(peer, snapshot, effective_depth)| {
            (
                snapshot.root_address,
                *effective_depth,
                self.graph.address(*peer).ok(),
                *peer,
            )
        });
        let (best_peer, best, best_effective_depth) = candidates[0];
        if own_address <= best.root_address {
            if self.graph.parent(node).is_some() || self.graph.root(node) != node {
                return Ok(Some(Transition {
                    parent: None,
                    ancestry: vec![node],
                    mandatory: true,
                }));
            }
            return Ok(None);
        }
        let mut ancestry = Vec::with_capacity(best.ancestry.len() + 1);
        ancestry.push(node);
        ancestry.extend_from_slice(&best.ancestry);
        if self.graph.ancestry(node) == ancestry {
            return Ok(None);
        }
        let current_root_address = self.graph.address(self.graph.root(node))?;
        let mandatory = best.root_address < current_root_address
            || self.graph.parent(node).is_none()
            || self.graph.parent(node).is_some_and(|parent| {
                !self.graph.is_active(parent) || !self.peer_views.contains_key(&(node, parent))
            });
        if !mandatory {
            let hold_down = self.last_parent_switch_ns[node as usize].is_some_and(|last| {
                self.scheduler.now_ns().saturating_sub(last) < self.config.parent_hold_down_ns
            });
            if hold_down {
                return Ok(None);
            }
            let Some(current_parent) = self.graph.parent(node) else {
                return Ok(None);
            };
            let Some(current_snapshot) = self.peer_views.get(&(node, current_parent)) else {
                return Ok(None);
            };
            let current_effective_depth =
                self.parent_effective_depth_ppm(node, current_parent, current_snapshot)?;
            let threshold = u128::from(current_effective_depth).saturating_mul(u128::from(
                1_000_000_u64.saturating_sub(u64::from(self.config.parent_hysteresis_ppm)),
            )) / 1_000_000;
            if u128::from(best_effective_depth) >= threshold {
                return Ok(None);
            }
        }
        Ok(Some(Transition {
            parent: Some(best_peer),
            ancestry,
            mandatory,
        }))
    }

    pub(super) fn apply_transition(
        &mut self,
        node: NodeId,
        transition: Transition,
        cause: &str,
        parent_event: Option<EventId>,
        evidence: &str,
    ) -> Result<(), RunError> {
        let old_parent = self.graph.parent(node);
        let old_root_address = self.graph.address(self.graph.root(node))?;
        self.graph
            .set_tree(node, transition.parent, transition.ancestry)?;
        self.last_parent_switch_ns[node as usize] = Some(self.scheduler.now_ns());
        if old_parent != self.graph.parent(node) {
            self.parent_transitions += 1;
            if let Some(runtime) = self.recovery.as_mut() {
                let invalidated = runtime.invalidate_path_node(node);
                self.add_ledger(cause, "cache-invalidated", invalidated, evidence);
            }
        }
        let root_address = self.graph.address(self.graph.root(node))?;
        if old_root_address != root_address {
            if let Some(runtime) = self.recovery.as_mut() {
                let invalidated = runtime.invalidate_root(old_root_address.0);
                let disrupted = runtime.disrupt_sessions_for_node(node);
                self.add_ledger(cause, "cache-invalidated", invalidated, evidence);
                self.add_ledger(cause, "sessions-disrupted", disrupted, evidence);
            }
        }
        self.root_generations.insert(root_address);
        self.add_ledger(cause, "state-mutated", 1, evidence);
        self.request_all(node, cause, parent_event)?;
        self.request_bloom_all(node, cause, parent_event)?;
        let _mandatory = transition.mandatory;
        Ok(())
    }

    pub(super) fn parent_effective_depth_ppm(
        &self,
        node: NodeId,
        peer: NodeId,
        snapshot: &TreeSnapshot,
    ) -> Result<u64, RunError> {
        let edge = self.graph.edge_between(node, peer).ok_or_else(|| {
            RunError::Invariant(format!("parent candidate {peer} is not adjacent to {node}"))
        })?;
        let peer_depth = snapshot.ancestry.len().saturating_sub(1) as u64;
        Ok(peer_depth
            .saturating_mul(1_000_000)
            .saturating_add(self.parent_cost_ppm[edge as usize]))
    }

    pub(super) fn snapshot(&self, node: NodeId) -> Result<TreeSnapshot, RunError> {
        let root = self.graph.root(node);
        Ok(TreeSnapshot {
            root,
            root_address: self.graph.address(root)?,
            parent: self.graph.parent(node),
            sequence: self.graph.sequence(node),
            ancestry: self.graph.ancestry(node).to_vec(),
        })
    }

    pub(super) fn snapshot_semantics_valid(
        &self,
        snapshot: &TreeSnapshot,
    ) -> Result<bool, RunError> {
        let unique = snapshot.ancestry.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != snapshot.ancestry.len()
            || snapshot.ancestry.last() != Some(&snapshot.root)
            || snapshot.parent != snapshot.ancestry.get(1).copied()
        {
            return Ok(false);
        }
        let minimum = snapshot
            .ancestry
            .iter()
            .map(|node| self.graph.address(*node))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min();
        Ok(minimum == Some(snapshot.root_address))
    }

    pub(super) fn add_ledger(&mut self, cause: &str, stage: &str, count: u64, evidence: &str) {
        let entry = self
            .ledger
            .entry((cause.to_owned(), stage.to_owned()))
            .or_default();
        entry.count = entry.count.saturating_add(count);
        if !evidence.is_empty() {
            entry.evidence.push(evidence.to_owned());
        }
    }
}
