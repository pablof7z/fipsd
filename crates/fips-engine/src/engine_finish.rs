use super::*;

impl Simulation {
    pub(super) fn finish(mut self) -> Result<IndividualRun, RunError> {
        if !self.pending.is_empty() || !self.scheduler.is_empty() {
            return Err(RunError::Invariant(
                "quiescence reached with pending events".to_owned(),
            ));
        }
        self.links.reconcile()?;
        let assertions = self.evaluate_invariants()?;
        if let Some(failed) = assertions.iter().find(|result| result.outcome != "pass") {
            return Err(RunError::Invariant(format!(
                "{}: {}",
                failed.id, failed.message
            )));
        }
        let fidelity = FidelityContract {
            wire: WireFidelity::ExecutableCodec,
            protocol: ProtocolFidelity::SemanticExact,
            compute: ComputeFidelity::OperationCounted,
            scale: ScaleFidelity::Individual,
            bloom: BloomFidelity::ExactBits,
            represented_nodes: self.config.nodes.into(),
            approximations: Vec::new(),
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
            arrivals: self.accepted_arrivals,
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
            quiescence_ns: self.scheduler.now_ns(),
            tree_announce: self.tree.clone(),
            links: self.links.all_counters(),
            scheduler: self.scheduler.diagnostics().clone(),
            graph_memory: self.graph.memory_footprint(),
            assertions: assertions.clone(),
        };
        let metric_series = vec![
            metric(
                "root.generations",
                "count",
                self.scheduler.now_ns(),
                report.root_generations.len() as u64,
            ),
            metric(
                "tree.maximum-depth",
                "edges",
                self.scheduler.now_ns(),
                maximum_depth,
            ),
            metric(
                "tree.parent-transitions",
                "count",
                self.scheduler.now_ns(),
                self.parent_transitions,
            ),
            metric(
                "tree-announce.transmitted-bytes",
                "bytes",
                self.scheduler.now_ns(),
                self.tree.transmitted_frame_bytes,
            ),
            metric(
                "quiescence",
                "nanoseconds",
                self.scheduler.now_ns(),
                self.scheduler.now_ns(),
            ),
        ];
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
        let checks = [
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
