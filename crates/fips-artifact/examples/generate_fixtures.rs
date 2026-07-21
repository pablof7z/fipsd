use fips_artifact::{
    AssertionResult, BloomFidelity, ComputeFidelity, EventRecord, ExternalBlob, FidelityContract,
    LedgerEntry, M0_FIPS_COMMIT, MetricPoint, MetricSeries, ProtocolFidelity, ProvenanceEnvelope,
    REPRODUCTION_BUNDLE_VERSION, RUN_ARTIFACT_VERSION, ReproductionBundle, RunArtifact,
    RunManifest, ScaleFidelity, WireFidelity,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("fixtures/artifacts");
    let blobs = directory.join("blobs");
    fs::create_dir_all(&blobs).unwrap();

    let external_bytes = b"[{\"virtual_time_ns\":0,\"value\":\"0\"},{\"virtual_time_ns\":500000000,\"value\":\"10\"}]\n";
    fs::write(blobs.join("root-agreement.json"), external_bytes).unwrap();
    let external_blob = ExternalBlob {
        role: "metric-series".to_owned(),
        path: "blobs/root-agreement.json".to_owned(),
        sha256: format!("{:x}", Sha256::digest(external_bytes)),
        size_bytes: external_bytes.len() as u64,
        encoding: "identity".to_owned(),
    };

    let plan_bytes = fs::read(root.join("fixtures/normalized/root-ratchet.json")).unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&plan_bytes).unwrap();
    let plan_sha = format!("{:x}", Sha256::digest(&plan_bytes));
    let fidelity = FidelityContract {
        wire: WireFidelity::ExecutableCodec,
        protocol: ProtocolFidelity::SemanticExact,
        compute: ComputeFidelity::OperationCounted,
        scale: ScaleFidelity::Individual,
        bloom: BloomFidelity::ExactBits,
        represented_nodes: 10,
        approximations: Vec::new(),
        sampled_regions: Vec::new(),
    };
    let mut schema_versions = BTreeMap::new();
    schema_versions.insert(
        "campaign".to_owned(),
        "experiments.fips.network/v1alpha1".to_owned(),
    );
    schema_versions.insert(
        "normalized-plan".to_owned(),
        "experiments.fips.network/normalized-plan/v1alpha1".to_owned(),
    );
    schema_versions.insert("run-artifact".to_owned(), RUN_ARTIFACT_VERSION.to_owned());
    let provenance = ProvenanceEnvelope {
        engine_name: "m0-fixture".to_owned(),
        engine_version: "0.1.0".to_owned(),
        engine_source_revision: "1111111111111111111111111111111111111111".to_owned(),
        schema_versions,
        seed: 424242,
        normalized_plan_sha256: plan_sha,
        fips_commit: Some(M0_FIPS_COMMIT.to_owned()),
        image_digest: None,
        hardware_profile: None,
    };
    let event_trace = vec![
        EventRecord {
            event_id: "input-0001".to_owned(),
            virtual_time_ns: 0,
            ordinal: 0,
            kind: "campaign-start".to_owned(),
            causal_parent: None,
            data: json!({"seed": 424242}),
        },
        EventRecord {
            event_id: "input-0002".to_owned(),
            virtual_time_ns: 500_000_000,
            ordinal: 0,
            kind: "lower-root-arrival".to_owned(),
            causal_parent: Some("input-0001".to_owned()),
            data: json!({"arrival": 1}),
        },
    ];
    let artifact = RunArtifact {
        manifest: RunManifest {
            api_version: RUN_ARTIFACT_VERSION.to_owned(),
            artifact_id: "root-ratchet-m0-fixture".to_owned(),
            run_id: "root-ratchet-seed-424242".to_owned(),
            fidelity: fidelity.clone(),
            provenance: provenance.clone(),
        },
        normalized_plan: plan.clone(),
        event_trace,
        metric_series: vec![MetricSeries {
            name: "root-agreement".to_owned(),
            unit: "nodes".to_owned(),
            points: vec![MetricPoint {
                virtual_time_ns: 0,
                value: "0".to_owned(),
            }],
        }],
        causal_ledger: vec![LedgerEntry {
            causal_id: "input-0002".to_owned(),
            stage: "serialized".to_owned(),
            count: 1,
            evidence: vec!["frame-tree-depth-0".to_owned()],
        }],
        assertion_results: vec![AssertionResult {
            id: "same-seed-same-event-order-and-result".to_owned(),
            outcome: "pass".to_owned(),
            message: "fixture event order is deterministic".to_owned(),
        }],
        samples: vec![json!({"nodes": [0, 1], "edges": [[0, 1]]})],
        logs: vec![json!({"level": "info", "event_id": "input-0001"})],
        external_blobs: vec![external_blob.clone()],
    };
    write_json(&directory.join("root-ratchet-run.json"), &artifact);
    write_json(
        &directory.join("root-ratchet-manifest.json"),
        &artifact.manifest,
    );
    let order: Vec<_> = artifact
        .event_trace
        .iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "ordinal": event.ordinal,
                "virtual_time_ns": event.virtual_time_ns
            })
        })
        .collect();
    write_json(&directory.join("root-ratchet-event-order.json"), &order);

    let bundle = ReproductionBundle {
        api_version: REPRODUCTION_BUNDLE_VERSION.to_owned(),
        bundle_id: "root-ratchet-m0-reproduction".to_owned(),
        normalized_plan: plan,
        seed: 424242,
        engine: "m0-fixture".to_owned(),
        variant: "fips-80c956a-baseline".to_owned(),
        fidelity,
        provenance,
        expected_assertions: vec!["same-seed-same-event-order-and-result".to_owned()],
        external_blobs: vec![external_blob],
    };
    write_json(&directory.join("root-ratchet-reproduction.json"), &bundle);
}

fn write_json(path: &Path, value: &impl Serialize) {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    fs::write(path, bytes).unwrap();
}
