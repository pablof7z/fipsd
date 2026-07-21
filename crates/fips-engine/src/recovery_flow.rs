use super::*;

impl<'a> CoupledState<'a> {
    pub(super) fn process_flow(
        &mut self,
        index: usize,
        offered_at_ns: u64,
    ) -> Result<(), RecoveryError> {
        let flow = self.traffic_plan.flows[index].clone();
        let flow_parent = self.latest_arrival_cause.clone();
        self.ledger.push(ledger_child(
            &flow.id,
            flow_parent.as_deref(),
            "performed",
            1,
            "traffic-flow",
        ));
        self.ledger.push(ledger_child(
            &flow.id,
            flow_parent.as_deref(),
            "payload",
            flow.useful_payload_bytes,
            "application-offer",
        ));
        self.traffic.offered_flows += 1;
        self.traffic.offered_useful_bytes = self
            .traffic
            .offered_useful_bytes
            .saturating_add(flow.useful_payload_bytes);
        let causal_id = flow.id.clone();
        let mut ready_at = offered_at_ns;
        if self
            .cache
            .get(&node_key(flow.destination), offered_at_ns)
            .is_none()
        {
            if self.lookup.coords_required(offered_at_ns) {
                *self
                    .lookup_counters
                    .signals
                    .entry(RoutingSignal::CoordsRequired)
                    .or_default() += 1;
            } else {
                self.lookup_counters.signals_rate_limited += 1;
            }
            let depth = flow.source.abs_diff(flow.destination);
            let lookup = self.lookup.execute(
                &LookupCase {
                    seed: self.seed,
                    ordinal: index as u64,
                    source: flow.source,
                    destination: flow.destination,
                    required_hops: depth,
                    origin_depth: flow.source.min(64),
                    target_depth: flow.destination.min(64),
                    false_positive_candidates: if self.bloom_model.contains_seeded(
                        &flow.destination.to_le_bytes(),
                        self.seed,
                        index as u64,
                        true,
                    ) {
                        0
                    } else {
                        1
                    },
                    reverse_path_broken: false,
                    target_absent: false,
                },
                offered_at_ns,
            );
            if let Some(first) = lookup.attempts.first() {
                self.ledger.push(ledger_child(
                    &first.causal_id,
                    Some(&flow.id),
                    "performed",
                    1,
                    "logical-lookup",
                ));
            }
            merge_lookup(&mut self.lookup_counters, &lookup.counters);
            ready_at = lookup
                .attempts
                .last()
                .map_or(ready_at, |attempt| attempt.started_at_ns);
            for attempt in &lookup.attempts {
                let attempt_parent = attempt
                    .parent_causal_id
                    .as_deref()
                    .unwrap_or(flow.id.as_str());
                self.ledger.push(ledger_child(
                    &attempt.causal_id,
                    Some(attempt_parent),
                    "requested",
                    1,
                    "lookup-attempt",
                ));
                let request_frame = attempt.request_message_bytes + ESTABLISHED_FMP_OVERHEAD_BYTES;
                self.enqueue_frame(
                    &attempt.causal_id,
                    Some(attempt_parent),
                    "lookup-request",
                    LinkClass::Control,
                    request_frame,
                    0,
                    attempt.started_at_ns,
                    depth,
                )?;
                if attempt.response_message_bytes > 0 {
                    self.enqueue_frame(
                        &attempt.causal_id,
                        Some(attempt_parent),
                        "lookup-response",
                        LinkClass::Control,
                        attempt.response_message_bytes + ESTABLISHED_FMP_OVERHEAD_BYTES,
                        0,
                        attempt.started_at_ns,
                        depth,
                    )?;
                }
            }
            self.last_lookup_ns = self.last_lookup_ns.max(self.link_delivery_ns.max(ready_at));
            if lookup.outcome != LookupOutcome::Success {
                self.starve(flow.useful_payload_bytes);
                return Ok(());
            }
            self.cache.insert(
                node_key(flow.destination),
                self.current_root,
                chain_path(flow.destination),
                ready_at,
            );
            self.consume(
                flow.source,
                &causal_id,
                ResourceKind::CacheEntries,
                1,
                ready_at,
            );
            self.consume(
                flow.source,
                &causal_id,
                ResourceKind::Verifications,
                1,
                ready_at,
            );
        }
        match flow.session_action {
            SessionAction::Setup => {
                self.traffic.session_setups += 1;
                self.traffic.setup_message_bytes =
                    self.traffic.setup_message_bytes.saturating_add(176);
                self.consume(flow.source, &causal_id, ResourceKind::Sessions, 1, ready_at);
                self.consume(
                    flow.source,
                    &causal_id,
                    ResourceKind::Handshakes,
                    3,
                    ready_at,
                );
                self.enqueue_frame(
                    &causal_id,
                    flow_parent.as_deref(),
                    "session-setup",
                    LinkClass::Control,
                    76 + ESTABLISHED_FMP_OVERHEAD_BYTES,
                    0,
                    ready_at,
                    flow.source.abs_diff(flow.destination),
                )?;
                self.enqueue_frame(
                    &causal_id,
                    flow_parent.as_deref(),
                    "session-ack",
                    LinkClass::Control,
                    100 + ESTABLISHED_FMP_OVERHEAD_BYTES,
                    0,
                    ready_at,
                    flow.source.abs_diff(flow.destination),
                )?;
            }
            SessionAction::Teardown => {
                self.traffic.session_teardowns += 1;
                self.resources[flow.source as usize].release(ResourceKind::Sessions, 1);
            }
            SessionAction::Rekey => {
                self.traffic.rekeys += 1;
                self.consume(
                    flow.source,
                    &causal_id,
                    ResourceKind::Handshakes,
                    1,
                    ready_at,
                );
            }
            SessionAction::Reuse => {}
        }
        self.consume(flow.source, &causal_id, ResourceKind::Hashes, 1, ready_at);
        self.consume(
            flow.source,
            &causal_id,
            ResourceKind::QueueBytes,
            flow.useful_payload_bytes,
            ready_at,
        );
        let before_delivered = self.link_delivery_ns;
        let delivered = self.enqueue_frame(
            &causal_id,
            flow_parent.as_deref(),
            "session-data",
            LinkClass::UsefulPayload,
            flow.useful_payload_bytes + DATA_FRAME_OVERHEAD_BYTES,
            flow.useful_payload_bytes,
            ready_at,
            flow.source.abs_diff(flow.destination),
        )?;
        self.resources[flow.source as usize]
            .release(ResourceKind::QueueBytes, flow.useful_payload_bytes);
        if delivered > 0 {
            self.traffic.delivered_flows += 1;
            self.traffic.delivered_useful_bytes = self
                .traffic
                .delivered_useful_bytes
                .saturating_add(delivered);
            let latency = self.link_delivery_ns.saturating_sub(offered_at_ns);
            self.traffic.maximum_latency_ns = self.traffic.maximum_latency_ns.max(latency);
            self.traffic.total_latency_ns = self.traffic.total_latency_ns.saturating_add(latency);
            self.last_useful_delivery_ns = Some(self.link_delivery_ns);
            self.useful_delivery_times.push(self.link_delivery_ns);
        } else {
            self.starve(flow.useful_payload_bytes);
            self.link_delivery_ns = before_delivered;
        }
        Ok(())
    }
}
