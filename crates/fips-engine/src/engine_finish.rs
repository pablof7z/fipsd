use super::*;

impl Simulation {
    pub(super) fn finish(mut self) -> Result<IndividualRun, RunError> {
        if !self.pending.is_empty()
            || self
                .bloom
                .as_ref()
                .is_some_and(|runtime| runtime.pending() > 0)
            || !self.scheduler.is_empty()
        {
            return Err(RunError::Invariant(
                "quiescence reached with pending events".to_owned(),
            ));
        }
        self.links.reconcile()?;
        let assertions = self.evaluate_invariants()?;
        let routed_traffic = self
            .traffic
            .as_ref()
            .map(|runtime| runtime.counters.clone());
        let bloom_propagation = self.bloom.as_ref().map(|runtime| runtime.counters.clone());
        let graph_recovery = self
            .recovery
            .as_ref()
            .map(GraphRecoveryRuntime::snapshot_counters);
        let root_tree_quiescence_ns = self
            .trace
            .iter()
            .filter(|event| {
                !event.kind.starts_with("data.")
                    && !event.kind.starts_with("bloom.")
                    && !event.kind.starts_with("lookup.")
                    && !event.kind.starts_with("session.")
            })
            .map(|event| event.virtual_time_ns)
            .max()
            .unwrap_or(0);
        let approximations =
            self.fidelity_approximations(routed_traffic.is_some(), graph_recovery.is_some());
        let fidelity = FidelityContract {
            wire: if routed_traffic.is_some() {
                WireFidelity::Modeled
            } else {
                WireFidelity::ExecutableCodec
            },
            protocol: ProtocolFidelity::SemanticExact,
            compute: ComputeFidelity::OperationCounted,
            scale: ScaleFidelity::Individual,
            bloom: match self.bloom.as_ref().map(|runtime| runtime.mode()) {
                Some(crate::BloomMode::SparseBits) => BloomFidelity::SparseBits,
                Some(crate::BloomMode::Occupancy) => BloomFidelity::Occupancy,
                _ => BloomFidelity::ExactBits,
            },
            represented_nodes: self.config.nodes.into(),
            approximations,
            sampled_regions: Vec::new(),
        };
        let normalized_bytes = self.plan.to_canonical_json()?;
        let normalized_sha = hex::encode(Sha256::digest(&normalized_bytes));
        let mut schema_versions = BTreeMap::new();
        schema_versions.insert(
            "normalized-plan".to_owned(),
            NORMALIZED_PLAN_VERSION.to_owned(),
        );
        schema_versions.insert("run-artifact".to_owned(), RUN_ARTIFACT_VERSION.to_owned());
        schema_versions.insert(
            "reproduction-bundle".to_owned(),
            REPRODUCTION_BUNDLE_VERSION.to_owned(),
        );
        let provenance = ProvenanceEnvelope {
            engine_name: ENGINE_NAME.to_owned(),
            engine_version: ENGINE_VERSION.to_owned(),
            engine_source_revision: engine_source_revision(),
            schema_versions,
            seed: self.plan.seed,
            normalized_plan_sha256: normalized_sha,
            fips_commit: Some(FIPS_COMMIT.to_owned()),
            image_digest: None,
            hardware_profile: None,
        };
        let run_hash_input = serde_json::to_vec(&json!({
            "plan": self.plan,
            "trace": self.trace,
            "variant": "fips-80c956a-baseline"
        }))?;
        let run_hash = hex::encode(Sha256::digest(run_hash_input));
        let run_id = format!("run-{}", &run_hash[..24]);
        let artifact_id = format!("artifact-{}", &run_hash[..32]);
        let final_root = self.minimum_active_address()?.to_hex();
        let ledger = std::mem::take(&mut self.ledger)
            .into_iter()
            .map(|((causal_id, stage), accumulator)| LedgerEntry {
                causal_id,
                causal_parent: None,
                stage,
                count: accumulator.count,
                evidence: accumulator.evidence,
            })
            .collect::<Vec<_>>();
        let maximum_depth = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .map(|id| self.graph.ancestry(id).len().saturating_sub(1) as u64)
            .max()
            .unwrap_or(0);
        let report = RootRatchetReport {
            kind: "root-ratchet-report/v1alpha1".to_owned(),
            run_id: run_id.clone(),
            seed: self.plan.seed,
            upstream_fips_commit: FIPS_COMMIT.to_owned(),
            fidelity_statement: fidelity.plain_language_statement(),
            graph_sha256: self.graph.graph_sha256(),
            node_count: self.config.nodes.into(),
            transport_profiles: self.transports.profile_counts(),
            routed_traffic: routed_traffic.clone(),
            bloom_propagation: bloom_propagation.clone(),
            graph_recovery: graph_recovery.clone(),
            arrivals: self.accepted_arrivals,
            authenticated_sybil_arrivals: self.authenticated_sybil_arrivals,
            identity_generation_trials: self.identity_trials,
            final_root,
            root_generations: self
                .root_generations
                .iter()
                .copied()
                .map(NodeAddress::to_hex)
                .collect(),
            maximum_depth,
            parent_transitions: self.parent_transitions,
            quiescence_ns: root_tree_quiescence_ns,
            tree_announce: self.tree.clone(),
            links: self.links.all_counters(),
            scheduler: self.scheduler.diagnostics().clone(),
            graph_memory: self.graph.memory_footprint(),
            assertions: assertions.clone(),
        };
        let mut metric_series = vec![
            metric(
                "root.generations",
                "count",
                root_tree_quiescence_ns,
                report.root_generations.len() as u64,
            ),
            metric(
                "tree.maximum-depth",
                "edges",
                root_tree_quiescence_ns,
                maximum_depth,
            ),
            metric(
                "tree.parent-transitions",
                "count",
                root_tree_quiescence_ns,
                self.parent_transitions,
            ),
            metric(
                "adversary.authenticated-sybil-arrivals",
                "count",
                root_tree_quiescence_ns,
                self.authenticated_sybil_arrivals,
            ),
            metric(
                "tree-announce.transmitted-bytes",
                "bytes",
                root_tree_quiescence_ns,
                self.tree.transmitted_frame_bytes,
            ),
            metric(
                "quiescence",
                "nanoseconds",
                root_tree_quiescence_ns,
                root_tree_quiescence_ns,
            ),
        ];
        if let Some(traffic) = &routed_traffic {
            metric_series.push(metric(
                "traffic.useful-bytes-delivered",
                "bytes",
                traffic.quiescence_ns,
                traffic.delivered_useful_bytes,
            ));
        }
        if let Some(bloom) = &bloom_propagation {
            metric_series.push(metric(
                "bloom.filter-announce-delivered",
                "count",
                bloom.quiescence_ns,
                bloom.delivered_frames,
            ));
        }
        if let Some(recovery) = &graph_recovery {
            metric_series.push(metric(
                "lookup.successes",
                "count",
                recovery.quiescence_ns,
                recovery.successes,
            ));
            metric_series.push(metric(
                "session.setups",
                "count",
                recovery.quiescence_ns,
                recovery.session_setups,
            ));
        }
        let artifact = RunArtifact {
            manifest: RunManifest {
                api_version: RUN_ARTIFACT_VERSION.to_owned(),
                artifact_id,
                run_id: run_id.clone(),
                fidelity: fidelity.clone(),
                provenance: provenance.clone(),
            },
            normalized_plan: serde_json::to_value(&self.plan)?,
            event_trace: self.trace,
            metric_series,
            causal_ledger: ledger,
            assertion_results: assertions,
            samples: vec![serde_json::to_value(&report)?],
            logs: Vec::new(),
            external_blobs: Vec::new(),
        };
        artifact.validate()?;
        let reproduction = ReproductionBundle {
            api_version: REPRODUCTION_BUNDLE_VERSION.to_owned(),
            bundle_id: format!("bundle-{}", &run_hash[..32]),
            normalized_plan: serde_json::to_value(&self.plan)?,
            seed: self.plan.seed,
            engine: ENGINE_NAME.to_owned(),
            variant: "fips-80c956a-baseline".to_owned(),
            fidelity,
            provenance,
            expected_assertions: report
                .assertions
                .iter()
                .map(|assertion| assertion.id.clone())
                .collect(),
            external_blobs: Vec::new(),
        };
        Ok(IndividualRun {
            artifact,
            reproduction,
            report,
            recovery_report: None,
        })
    }
}
