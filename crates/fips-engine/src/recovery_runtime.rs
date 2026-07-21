use super::*;

impl<'a> CoupledState<'a> {
    pub(super) fn run(mut self) -> Result<RecoveryReport, RecoveryError> {
        let mut events = Vec::new();
        for arrival in 0..self.config.arrivals {
            let at = self
                .config
                .arrival_start_ns
                .saturating_add(u64::from(arrival).saturating_mul(self.config.arrival_interval_ns));
            events.push((at, 0_u8, u64::from(arrival), WorkEvent::Arrival(arrival)));
            events.push((
                at.saturating_add(self.config.tree_recovery_ns),
                1_u8,
                u64::from(arrival),
                WorkEvent::BloomArrival(arrival),
            ));
        }
        let flow_start = self.config.arrival_start_ns.saturating_sub(100_000_000);
        for (index, flow) in self.traffic_plan.flows.iter().enumerate() {
            events.push((
                flow_start.saturating_add(flow.offered_at_ns),
                2,
                index as u64,
                WorkEvent::Flow(index),
            ));
        }
        events.sort_by_key(|(at, priority, ordinal, _)| (*at, *priority, *ordinal));
        let mut bloom_tx_index = 0;
        for (at, _, _, event) in events {
            self.bloom_wave.flush(
                at,
                self.config.bloom_max_fpr,
                self.bloom_model.fpr(),
                self.config
                    .link
                    .mtu_bytes
                    .saturating_sub(self.config.link.transport_overhead_bytes),
            );
            self.enqueue_new_bloom(&mut bloom_tx_index)?;
            match event {
                WorkEvent::Arrival(ordinal) => self.process_arrival(ordinal, at)?,
                WorkEvent::BloomArrival(ordinal) => self.process_bloom_arrival(ordinal, at),
                WorkEvent::Flow(index) => self.process_flow(index, at)?,
            }
        }
        let final_flush = self
            .config
            .arrival_start_ns
            .saturating_add(
                u64::from(self.config.arrivals.saturating_sub(1))
                    .saturating_mul(self.config.arrival_interval_ns),
            )
            .saturating_add(self.config.tree_recovery_ns)
            .saturating_add(self.config.bloom_debounce_ns);
        self.bloom_wave.flush(
            final_flush,
            self.config.bloom_max_fpr,
            self.bloom_model.fpr(),
            self.config
                .link
                .mtu_bytes
                .saturating_sub(self.config.link.transport_overhead_bytes),
        );
        self.enqueue_new_bloom(&mut bloom_tx_index)?;
        self.link.reconcile()?;
        self.finish(final_flush)
    }

    pub(super) fn process_arrival(
        &mut self,
        ordinal: u32,
        at_ns: u64,
    ) -> Result<(), RecoveryError> {
        let causal_id = format!("input:arrival-{ordinal:04}");
        self.ledger
            .push(ledger(&causal_id, "performed", 1, "root-arrival"));
        let invalidated = self
            .cache
            .invalidate(&Invalidation::Root(self.current_root));
        for pool in &mut self.resources {
            pool.release(ResourceKind::CacheEntries, invalidated);
        }
        self.current_root = self
            .root
            .root_generations
            .get(ordinal as usize + 1)
            .or_else(|| self.root.root_generations.last())
            .map(|value| hex_16(value))
            .transpose()?
            .unwrap_or(self.current_root);
        self.ledger
            .push(ledger(&causal_id, "state-mutated", 1, "root-generation"));
        self.ledger.push(ledger(
            &causal_id,
            "state-mutated",
            invalidated,
            "coordinate-cache-invalidations",
        ));
        self.arrival_summaries.push(ArrivalAmplification {
            causal_id: causal_id.clone(),
            at_ns,
            bloom_requests: 0,
            bloom_frames: 0,
            bloom_fmp_bytes: 0,
            cache_invalidations: invalidated,
            compute_units: 0,
        });
        self.latest_arrival_cause = Some(causal_id);
        Ok(())
    }

    pub(super) fn process_bloom_arrival(&mut self, ordinal: u32, at_ns: u64) {
        let causal_id = format!("input:arrival-{ordinal:04}");
        let directions = u64::from(self.config.nodes.saturating_sub(1)).saturating_mul(2);
        for peer in 0..u32::try_from(directions).unwrap_or(u32::MAX) {
            let role = if peer % 11 == 0 {
                PeerRole::Mesh
            } else if peer % 2 == 0 {
                PeerRole::Parent
            } else {
                PeerRole::Child
            };
            self.bloom_wave
                .request_causal(peer, role, at_ns, &causal_id);
        }
        let units = 5 + (self.config.nodes as u64).div_ceil(8);
        let compute = units.saturating_mul(u64::from(self.config.nodes));
        for node in 0..self.config.nodes {
            self.consume(
                node,
                &causal_id,
                ResourceKind::BloomOperations,
                units,
                at_ns,
            );
        }
        if let Some(summary) = self
            .arrival_summaries
            .iter_mut()
            .find(|summary| summary.causal_id == causal_id)
        {
            summary.bloom_requests = directions;
            summary.compute_units = compute;
        }
    }
}
