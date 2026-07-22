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
                    .map(|snapshot| (peer, snapshot))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(None);
        }
        candidates.sort_by_key(|(peer, snapshot)| {
            (
                snapshot.root_address,
                snapshot.ancestry.len(),
                self.graph.address(*peer).ok(),
                *peer,
            )
        });
        let (best_peer, best) = candidates[0];
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
            || self
                .graph
                .parent(node)
                .is_some_and(|parent| !self.graph.is_active(parent));
        if !mandatory {
            let hold_down = self.last_parent_switch_ns[node as usize].is_some_and(|last| {
                self.scheduler.now_ns().saturating_sub(last) < self.config.parent_hold_down_ns
            });
            if hold_down {
                return Ok(None);
            }
            let current_depth = self.graph.ancestry(node).len() as u64;
            let candidate_depth = ancestry.len() as u64;
            let threshold = current_depth.saturating_mul(
                1_000_000_u64.saturating_sub(u64::from(self.config.parent_hysteresis_ppm)),
            ) / 1_000_000;
            if candidate_depth >= threshold {
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
        self.graph
            .set_tree(node, transition.parent, transition.ancestry)?;
        self.last_parent_switch_ns[node as usize] = Some(self.scheduler.now_ns());
        if old_parent != self.graph.parent(node) {
            self.parent_transitions += 1;
        }
        let root_address = self.graph.address(self.graph.root(node))?;
        self.root_generations.insert(root_address);
        self.add_ledger(cause, "state-mutated", 1, evidence);
        self.request_all(node, cause, parent_event)?;
        let _mandatory = transition.mandatory;
        Ok(())
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
