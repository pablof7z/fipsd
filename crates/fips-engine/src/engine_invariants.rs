use super::*;

impl Simulation {
    pub(super) fn evaluate_invariants(&self) -> Result<Vec<AssertionResult>, RunError> {
        let active = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .collect::<Vec<_>>();
        let minimum = self.minimum_active_address()?;
        let root_agreement = active.iter().all(|node| {
            self.graph
                .address(self.graph.root(*node))
                .is_ok_and(|address| address == minimum)
        });
        let loop_free = active.iter().all(|node| {
            let path = self.graph.ancestry(*node);
            path.iter().copied().collect::<BTreeSet<_>>().len() == path.len()
        });
        let coordinate_consistent = active.iter().all(|node| {
            let path = self.graph.ancestry(*node);
            path.first() == Some(node)
                && path.last() == Some(&self.graph.root(*node))
                && self.graph.parent(*node) == path.get(1).copied()
        });
        let debounce = self.sent_times.values().all(|times| {
            times
                .windows(2)
                .all(|pair| pair[1].saturating_sub(pair[0]) >= self.config.debounce_ns)
        });
        let queues = self.links.all_counters().values().all(|counters| {
            counters.transmitted_bytes == counters.delivered_bytes + counters.lost_bytes
        });
        let lifecycle = self.tree.requested
            == self.tree.constructed + self.tree.superseded + self.tree.cancelled
            && self.tree.superseded == self.tree.coalesced
            && self.tree.constructed == self.tree.serialized
            && self.tree.constructed == self.tree.queued + self.tree.rejected;
        let mut checks = vec![
            (
                "root-agreement",
                root_agreement,
                "all active nodes advertise the minimum active address",
            ),
            (
                "loop-freedom",
                loop_free,
                "every ancestry contains unique stable node IDs",
            ),
            (
                "no-obsolete-root-retention",
                root_agreement,
                "no active node retains a superseded root at quiescence",
            ),
            (
                "per-peer-debounce",
                debounce,
                "every transmitted per-peer announcement obeys the configured boundary",
            ),
            (
                "coordinate-consistency",
                coordinate_consistent,
                "parent, root, and ancestry columns agree",
            ),
            (
                "control-queues-return-to-baseline",
                queues,
                "all transmitted bytes are delivered or deterministically lost",
            ),
            (
                "tree-lifecycle-reconciliation",
                lifecycle,
                "requested, coalesced, cancelled, constructed, serialized, queued, and rejected totals reconcile",
            ),
            (
                "byte-reconciliation",
                queues,
                "per-edge transmitted bytes equal delivered plus lost bytes",
            ),
            (
                "deterministic-total-order",
                self.trace.windows(2).all(|pair| {
                    (pair[0].virtual_time_ns, pair[0].ordinal, &pair[0].event_id)
                        <= (pair[1].virtual_time_ns, pair[1].ordinal, &pair[1].event_id)
                }),
                "event order is a stable virtual-time and ordinal total order",
            ),
        ];
        if let Some(traffic) = self.traffic.as_ref().map(|runtime| &runtime.counters) {
            checks.push((
                "routed-traffic-reconciliation",
                traffic.offered_flows == traffic.delivered_flows + traffic.rejected_flows
                    && traffic.offered_useful_bytes
                        == traffic.delivered_useful_bytes + traffic.lost_useful_bytes,
                "offered routed flows and useful bytes reconcile to delivered and rejected mass",
            ));
        }
        if let Some(bloom) = self.bloom.as_ref().map(|runtime| &runtime.counters) {
            checks.push((
                "bloom-propagation-reconciliation",
                bloom.reconciles(),
                "Bloom requests, coalescing, construction, rejection, wire loss, and delivery reconcile",
            ));
        }
        if let Some(recovery) = self.recovery.as_ref() {
            checks.push((
                "graph-recovery-reconciliation",
                recovery.snapshot_counters().reconciles(),
                "logical lookup outcomes, retries, and recovery wire bytes reconcile",
            ));
        }
        Ok(checks
            .into_iter()
            .map(|(id, passed, message)| AssertionResult {
                id: id.to_owned(),
                outcome: if passed { "pass" } else { "fail" }.to_owned(),
                message: message.to_owned(),
            })
            .collect())
    }

    pub(super) fn minimum_active_address(&self) -> Result<NodeAddress, RunError> {
        self.graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .map(|id| self.graph.address(id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min()
            .ok_or_else(|| RunError::Invariant("no active nodes".to_owned()))
    }
}
