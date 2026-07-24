use fips_artifact::{ReproductionBundle, RunArtifact};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn pretty(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn golden_run_artifact_round_trips_without_loss() {
    let root = workspace_root();
    let bytes = fs::read(root.join("fixtures/artifacts/root-ratchet-run.json")).unwrap();
    let artifact: RunArtifact = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(artifact.to_canonical_json().unwrap(), bytes);
    for blob in &artifact.external_blobs {
        blob.verify(&root.join("fixtures/artifacts")).unwrap();
    }
}

#[test]
fn golden_reproduction_round_trips_without_loss() {
    let root = workspace_root();
    let bytes = fs::read(root.join("fixtures/artifacts/root-ratchet-reproduction.json")).unwrap();
    let bundle: ReproductionBundle = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(bundle.to_canonical_json().unwrap(), bytes);
}

#[test]
fn manifest_and_event_order_sections_are_byte_stable() {
    let root = workspace_root();
    let artifact: RunArtifact = serde_json::from_slice(
        &fs::read(root.join("fixtures/artifacts/root-ratchet-run.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        pretty(&artifact.manifest),
        fs::read(root.join("fixtures/artifacts/root-ratchet-manifest.json")).unwrap()
    );
    let order: Vec<Value> = artifact
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
    assert_eq!(
        pretty(&order),
        fs::read(root.join("fixtures/artifacts/root-ratchet-event-order.json")).unwrap()
    );
}

#[test]
fn checked_in_documents_satisfy_json_schemas() {
    let root = workspace_root();
    let cases = [
        (
            "schemas/run-artifact-v1alpha1.schema.json",
            "fixtures/artifacts/root-ratchet-run.json",
        ),
        (
            "schemas/reproduction-bundle-v1alpha1.schema.json",
            "fixtures/artifacts/root-ratchet-reproduction.json",
        ),
        (
            "schemas/render-frame-v1alpha1.schema.json",
            "fixtures/renderer/render-frame-minimal.json",
        ),
    ];
    for (schema_path, fixture_path) in cases {
        let schema: Value =
            serde_json::from_slice(&fs::read(root.join(schema_path)).unwrap()).unwrap();
        let fixture: Value =
            serde_json::from_slice(&fs::read(root.join(fixture_path)).unwrap()).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let errors: Vec<_> = validator
            .iter_errors(&fixture)
            .map(|error| format!("{}: {error}", error.instance_path))
            .collect();
        assert!(errors.is_empty(), "{}: {}", fixture_path, errors.join("; "));
    }
}
