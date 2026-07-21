use fips_engine::IndividualEngine;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_accepts(schema_path: &Path, document: &Value) {
    let schema: Value = serde_json::from_slice(&fs::read(schema_path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator
        .iter_errors(document)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
}

#[test]
fn checked_in_m1_run_replays_bit_for_bit_and_validates() {
    let repository = repository();
    let plan =
        fips_model::normalize_path(&repository.join("examples/m1/root-ratchet-12.yaml")).unwrap();
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let artifact_bytes = run.artifact.to_canonical_json().unwrap();
    let reproduction_bytes = run.reproduction.to_canonical_json().unwrap();
    assert_eq!(
        artifact_bytes,
        fs::read(repository.join("fixtures/m1/root-ratchet-12-artifact.json")).unwrap()
    );
    assert_eq!(
        reproduction_bytes,
        fs::read(repository.join("fixtures/m1/root-ratchet-12-reproduction.json")).unwrap()
    );
    let replay = IndividualEngine.run_plan(&plan).unwrap();
    assert_eq!(artifact_bytes, replay.artifact.to_canonical_json().unwrap());
    schema_accepts(
        &repository.join("schemas/run-artifact-v1alpha1.schema.json"),
        &serde_json::to_value(&run.artifact).unwrap(),
    );
    schema_accepts(
        &repository.join("schemas/reproduction-bundle-v1alpha1.schema.json"),
        &serde_json::to_value(&run.reproduction).unwrap(),
    );
    assert!(
        run.artifact
            .assertion_results
            .iter()
            .all(|assertion| assertion.outcome == "pass")
    );
}

#[test]
fn checked_in_broken_fixture_fails_the_named_invariant() {
    let repository = repository();
    let plan =
        fips_model::normalize_path(&repository.join("examples/m1/root-ratchet-12-broken.yaml"))
            .unwrap();
    let error = IndividualEngine.run_plan(&plan).unwrap_err();
    assert!(error.to_string().contains("loop-freedom"));
}

#[test]
fn topology_generator_hashes_match_the_published_golden() {
    let repository = repository();
    let expected: std::collections::BTreeMap<String, String> = serde_json::from_slice(
        &fs::read(repository.join("fixtures/m1/topology-hashes.json")).unwrap(),
    )
    .unwrap();
    let cases = [
        ("chain", fips_engine::TopologyKind::Chain, 2),
        ("balanced-tree", fips_engine::TopologyKind::BalancedTree, 2),
        (
            "random-regular",
            fips_engine::TopologyKind::RandomRegular,
            4,
        ),
        ("scale-free", fips_engine::TopologyKind::ScaleFree, 4),
    ];
    for (name, kind, degree) in cases {
        let graph = fips_engine::GraphStore::generate(kind, 20, degree, 424242, &[]).unwrap();
        assert_eq!(&graph.graph_sha256(), &expected[name]);
    }
}
