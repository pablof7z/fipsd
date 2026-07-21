use crate::{CohortReport, ScaleMetrics};
use fips_artifact::{
    AssertionResult, EventRecord, LedgerEntry, M0_FIPS_COMMIT, MetricPoint, MetricSeries,
    ProvenanceEnvelope, RUN_ARTIFACT_VERSION, RunArtifact, RunManifest,
};
use fips_model::{NORMALIZED_PLAN_VERSION, NormalizedPlan};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub fn build_scale_artifact(
    plan: &NormalizedPlan,
    report: &CohortReport,
) -> Result<RunArtifact, fips_artifact::ArtifactError> {
    let hash_input = serde_json::to_vec(&json!({"plan": plan, "report": report}))
        .map_err(fips_artifact::ArtifactError::Json)?;
    let hash = hex::encode(Sha256::digest(hash_input));
    let run_id = format!("scale-run-{}", &hash[..24]);
    let normalized_sha256 = hex::encode(Sha256::digest(plan.to_canonical_json().map_err(
        |error| {
            fips_artifact::ArtifactError::Json(serde_json::Error::io(std::io::Error::other(
                error.to_string(),
            )))
        },
    )?));
    let provenance = ProvenanceEnvelope {
        engine_name: "fips-cohort-hybrid".to_owned(),
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        engine_source_revision: option_env!("FIPS_ENGINE_SOURCE_REVISION")
            .unwrap_or("workspace-uncommitted")
            .to_owned(),
        schema_versions: BTreeMap::from([
            (
                "normalized-plan".to_owned(),
                NORMALIZED_PLAN_VERSION.to_owned(),
            ),
            ("run-artifact".to_owned(), RUN_ARTIFACT_VERSION.to_owned()),
            ("scale-report".to_owned(), "v1alpha1".to_owned()),
        ]),
        seed: plan.seed,
        normalized_plan_sha256: normalized_sha256,
        fips_commit: Some(M0_FIPS_COMMIT.to_owned()),
        image_digest: None,
        hardware_profile: None,
    };
    let mut artifact = RunArtifact {
        manifest: RunManifest {
            api_version: RUN_ARTIFACT_VERSION.to_owned(),
            artifact_id: format!("scale-artifact-{}", &hash[..32]),
            run_id,
            fidelity: report.fidelity.clone(),
            provenance,
        },
        normalized_plan: serde_json::to_value(plan).map_err(fips_artifact::ArtifactError::Json)?,
        event_trace: vec![EventRecord {
            event_id: "cohort-transition-0000".to_owned(),
            virtual_time_ns: 0,
            ordinal: 0,
            kind: "cohort-root-ratchet".to_owned(),
            causal_parent: None,
            data: json!({"population": report.represented_nodes, "variant": report.variant}),
        }],
        metric_series: metric_series(&report.metrics),
        causal_ledger: vec![LedgerEntry {
            causal_id: "cohort-transition-0000".to_owned(),
            causal_parent: None,
            stage: "population-transition".to_owned(),
            count: report.represented_nodes,
            evidence: vec!["cohort-report".to_owned()],
        }],
        assertion_results: vec![
            assertion(
                "population-mass-conserved",
                report.population_before == report.population_after,
            ),
            assertion(
                "cohort-bounds-declared",
                report
                    .fidelity
                    .approximations
                    .iter()
                    .all(|item| !item.uncertainty.is_empty()),
            ),
        ],
        samples: vec![serde_json::to_value(report).map_err(fips_artifact::ArtifactError::Json)?],
        logs: vec![
            json!({"variant": report.variant, "fidelity": report.fidelity.plain_language_statement()}),
        ],
        external_blobs: Vec::new(),
    };
    artifact
        .event_trace
        .sort_by_key(|event| (event.virtual_time_ns, event.ordinal, event.event_id.clone()));
    artifact.validate()?;
    Ok(artifact)
}

fn metric_series(metrics: &ScaleMetrics) -> Vec<MetricSeries> {
    [
        ("root.adoptions", &metrics.root_adoptions),
        ("tree.parent-transitions", &metrics.parent_transitions),
        ("tree.maximum-depth", &metrics.maximum_depth),
        ("control.transmitted-bytes", &metrics.control_bytes),
        ("bloom.fpr", &metrics.bloom_fpr_ppb),
        ("queue.peak-bytes", &metrics.peak_queue_bytes),
        (
            "traffic.useful-payload-bytes",
            &metrics.useful_payload_bytes,
        ),
        ("quiescence", &metrics.quiescence_ns),
    ]
    .into_iter()
    .map(|(name, estimate)| MetricSeries {
        name: name.to_owned(),
        unit: estimate.unit.clone(),
        points: vec![MetricPoint {
            virtual_time_ns: 0,
            value: estimate.value.clone(),
        }],
    })
    .collect()
}

fn assertion(id: &str, pass: bool) -> AssertionResult {
    AssertionResult {
        id: id.to_owned(),
        outcome: if pass { "pass" } else { "fail" }.to_owned(),
        message: format!("{id}: {}", if pass { "satisfied" } else { "violated" }),
    }
}
