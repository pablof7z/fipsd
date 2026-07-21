use super::*;

impl<'a> CoupledState<'a> {
    pub(super) fn finish(mut self, final_flush: u64) -> Result<RecoveryReport, RecoveryError> {
        let shared_link = self.link.counters(0, 0, 1);
        let peak_queue_bytes = shared_link.peak_queue_bytes;
        self.projections
            .by_edge
            .insert("edge:0".to_owned(), shared_link.transmitted_bytes);
        let resources = aggregate_resources(&self.resources);
        self.projections.by_resource = resources.consumed.clone();
        let compute_units = resources
            .consumed
            .iter()
            .filter(|(kind, _)| kind.is_service())
            .map(|(_, units)| *units)
            .sum::<u64>();
        let markers = RecoveryMarkers {
            root_ns: self.root.quiescence_ns,
            tree_ns: self.root.quiescence_ns,
            bloom_ns: self
                .bloom_wave
                .transmissions
                .iter()
                .map(|transmission| transmission.at_ns)
                .max()
                .unwrap_or(final_flush)
                .max(self.root.quiescence_ns),
            lookup_ns: self.last_lookup_ns,
            throughput_ns: self.last_useful_delivery_ns.unwrap_or(final_flush),
        };
        let mut prior_delivery = self.config.arrival_start_ns;
        for delivery in &self.useful_delivery_times {
            if *delivery >= self.config.arrival_start_ns {
                self.traffic.goodput_stall_ns = self
                    .traffic
                    .goodput_stall_ns
                    .max(delivery.saturating_sub(prior_delivery));
                prior_delivery = *delivery;
            }
        }
        let duplicate_bytes = shared_link
            .transmitted_bytes
            .saturating_sub(self.logical_wire_bytes);
        let transport_bytes = self
            .logical_wire_bytes
            .saturating_sub(self.accepted_frame_bytes);
        let semantic_actions = u64::from(self.config.arrivals)
            + self.lookup_counters.lookups
            + self.traffic.offered_flows;
        let state_bytes = (self.bloom_model.num_bits() / 8) as u64 + self.cache.counters.peak_bytes;
        let time_ns = markers
            .root_ns
            .max(markers.bloom_ns)
            .max(markers.lookup_ns)
            .max(markers.throughput_ns);
        let superseded_bytes = self
            .bloom_wave
            .counters
            .coalesced
            .saturating_mul(FILTER_ANNOUNCE_FMP_BYTES);
        self.ledger.push(ledger_child(
            "aggregate:m2",
            Some("input:arrival-0000"),
            "state",
            state_bytes,
            "peak-bloom-plus-coordinate-cache-bytes",
        ));
        self.ledger.push(ledger_child(
            "aggregate:m2",
            Some("input:arrival-0000"),
            "time",
            time_ns,
            "critical-path-virtual-time",
        ));
        if superseded_bytes > 0 {
            self.ledger.push(ledger_child(
                "aggregate:m2",
                Some("input:arrival-0000"),
                "superseded",
                superseded_bytes,
                "coalesced-full-bloom-replacements",
            ));
        }
        let projections_reconcile = self.projections.by_message.values().sum::<u64>()
            == self.accepted_frame_bytes
            && self.projections.by_edge.values().sum::<u64>() == shared_link.transmitted_bytes
            && resource_receipts_reconcile(&self.resources, &self.projections.by_resource)
            && self.projections.by_depth_band.values().sum::<u64>() == self.accepted_frame_bytes;
        let ledger_reconcile = stage_total(&self.ledger, "performed") == semantic_actions
            && stage_total(&self.ledger, "payload") == self.traffic.offered_useful_bytes
            && stage_total(&self.ledger, "fmp") == self.accepted_frame_bytes
            && stage_total(&self.ledger, "transport") == transport_bytes
            && stage_total(&self.ledger, "network") == shared_link.transmitted_bytes
            && stage_total(&self.ledger, "reliability")
                == shared_link.lost_bytes.saturating_add(duplicate_bytes)
            && stage_total(&self.ledger, "useful-payload") == self.traffic.delivered_useful_bytes
            && stage_total(&self.ledger, "compute") == compute_units
            && stage_total(&self.ledger, "state") == state_bytes
            && stage_total(&self.ledger, "time") == time_ns
            && stage_total(&self.ledger, "lower-bound") == self.logical_wire_bytes
            && stage_total(&self.ledger, "duplicate") == duplicate_bytes
            && stage_total(&self.ledger, "retransmitted") == duplicate_bytes
            && stage_total(&self.ledger, "superseded") == superseded_bytes;
        let costs = LayeredCosts {
            semantic_actions,
            payload_bytes: self.traffic.offered_useful_bytes,
            fsp_bytes: stage_total(&self.ledger, "fsp"),
            fmp_bytes: self.accepted_frame_bytes,
            transport_bytes,
            network_bytes: shared_link.transmitted_bytes,
            reliability_bytes: shared_link.lost_bytes.saturating_add(duplicate_bytes),
            useful_payload_bytes: self.traffic.delivered_useful_bytes,
            compute_units,
            state_bytes,
            time_ns,
            lower_bound_bytes: self.logical_wire_bytes,
            duplicate_bytes,
            superseded_bytes,
            retransmitted_bytes: duplicate_bytes,
            amplification_ppm: ratio_ppm(
                shared_link.transmitted_bytes,
                self.traffic.delivered_useful_bytes,
            ),
            frames_reconcile: self.bloom_wave.reconciles()
                && self.accepted_frame_bytes == self.projections.by_message.values().sum::<u64>(),
            projections_reconcile,
            ledger_reconcile,
        };
        let control_to_useful_ppm = ratio_ppm(
            shared_link
                .delivered_bytes
                .saturating_sub(shared_link.useful_payload_bytes),
            shared_link.useful_payload_bytes,
        );
        let critical_path = critical_path(
            &resources,
            &shared_link,
            self.config.tree_recovery_ns,
            self.lookup_counters.recovery_time_ns,
            self.config.bloom_debounce_ns,
            self.traffic.maximum_latency_ns,
        );
        let flow_start = self.config.arrival_start_ns.saturating_sub(100_000_000);
        let post_root_offered = self
            .traffic_plan
            .flows
            .iter()
            .any(|flow| flow_start.saturating_add(flow.offered_at_ns) >= markers.root_ns);
        let data_progress = self.traffic.offered_flows == 0
            || if post_root_offered {
                self.useful_delivery_times
                    .iter()
                    .any(|delivery| *delivery >= markers.root_ns)
            } else {
                self.traffic.delivered_useful_bytes > 0
            };
        let assertions = vec![
            assertion(
                "bloom-replacement-reconciliation",
                self.bloom_wave.reconciles(),
                "Bloom requests, coalescing, construction, sends, rejections, and bytes reconcile",
            ),
            assertion(
                "causal-ledger-frame-reconciliation",
                costs.frames_reconcile && costs.projections_reconcile && costs.ledger_reconcile,
                "ledger layers equal displayed totals and message/edge/depth/resource projections reconcile",
            ),
            assertion(
                "cache-invalidation-scope",
                self.cache.counters.invalidations <= self.cache.counters.insertions,
                "cache invalidation remains bounded by inserted coordinate entries",
            ),
            assertion(
                "continuous-control-eventual-data-progress",
                data_progress,
                "valid control traffic does not starve every useful flow",
            ),
            assertion(
                "modeled-work-has-resource-receipts",
                !self.resources.iter().any(|pool| {
                    pool.counters.consumed.values().sum::<u64>() > 0 && pool.receipts.is_empty()
                }),
                "every executed modeled work item has a causal resource receipt",
            ),
        ];
        if let Some(failed) = assertions
            .iter()
            .find(|assertion| assertion.outcome != "pass")
        {
            return Err(RecoveryError::Invariant(format!(
                "{}: {}",
                failed.id, failed.message
            )));
        }
        let depth_band_adoption_ns = depth_adoption(self.root, markers.root_ns);
        let fidelity_statement = match self.config.bloom_mode {
            BloomMode::ExactBits => "individual protocol state with exact packed Bloom bits; executable production wire sizes; operation-counted compute".to_owned(),
            BloomMode::SparseBits => "individual protocol state with sparse exact Bloom bit indices; executable production wire sizes; operation-counted compute".to_owned(),
            BloomMode::Occupancy => "individual protocol state with seeded statistical Bloom occupancy; executable production wire sizes; operation-counted compute".to_owned(),
        };
        let bloom_estimated_cardinality = self
            .bloom_model
            .estimated_cardinality(self.config.bloom_max_fpr)
            .map(|value| value.round() as u64);
        Ok(RecoveryReport {
            kind: "root-ratchet-recovery-report/v1alpha1".to_owned(),
            run_id: self.root.run_id.clone(),
            bloom_mode: self.config.bloom_mode,
            fidelity_statement,
            markers,
            intermediate_roots: self.root.root_generations.len() as u64,
            depth_band_adoption_ns,
            bloom: self.bloom_wave.counters,
            bloom_fill_ppm: (self.bloom_model.fill_ratio() * 1_000_000.0).round() as u64,
            bloom_fpr_ppb: (self.bloom_model.fpr() * 1_000_000_000.0).round() as u64,
            bloom_estimated_cardinality,
            cache: self.cache.counters,
            lookup: self.lookup_counters,
            traffic: self.traffic,
            resources,
            resource_exhaustions: self.resource_exhaustions,
            shared_link,
            maximum_frame_bytes: self.maximum_frame_bytes,
            peak_queue_bytes,
            control_to_useful_ppm,
            costs,
            projections: self.projections,
            per_arrival: self.arrival_summaries,
            critical_path,
            assertions,
            causal_ledger: self.ledger,
        })
    }
}
