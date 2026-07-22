mod common;

use fips_oracle::{
    DifferentialReport, FuzzArtifact, HarnessBundle, ImportResult, NormalizedTelemetry,
    ObservationStatus, OracleClassification, OracleRunReport, OracleSuiteCase, PINNED_FIPS_COMMIT,
    import_chaos_yaml,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn checked_m5_fixtures_cover_import_compile_telemetry_diff_oracle_fuzz_and_suites() {
    let imported: ImportResult = fixture("imported-smoke.json");
    assert_eq!(imported.source.fips_commit, PINNED_FIPS_COMMIT);
    assert!(!imported.diagnostics.is_empty());
    let compiled: HarnessBundle = fixture("compiled-smoke-manifest.json");
    assert_eq!(compiled.fips_commit, PINNED_FIPS_COMMIT);
    assert_eq!(compiled.deterministic_identity_ids.len(), 10);
    let yaml: Value =
        serde_yaml::from_slice(&fs::read(generated().join("compiled-smoke.yaml")).unwrap())
            .unwrap();
    assert_eq!(
        yaml.pointer("/topology/num_nodes").and_then(Value::as_u64),
        Some(10)
    );

    let telemetry: NormalizedTelemetry = fixture("normalized-telemetry.json");
    assert_eq!(telemetry.nodes.len(), 1);
    assert_eq!(telemetry.nodes[0].parent.status, ObservationStatus::Unknown);
    assert_eq!(
        telemetry.nodes[0].root.raw_source.as_deref(),
        Some("tree:n01")
    );
    let differential: DifferentialReport = fixture("differential.json");
    assert_eq!(differential.first_divergent_transition, Some(0));
    assert_eq!(
        differential.classification,
        OracleClassification::ImplementationBug
    );

    let recorded: OracleRunReport = fixture("recorded-oracle-report.json");
    assert!(recorded.backend.contains("recorded-fixture-not-live"));
    assert!(recorded.stable);
    assert_eq!(
        recorded.dominant_classification,
        OracleClassification::ExactMatch
    );
    let fuzz: FuzzArtifact = fixture("fuzz-crash.json");
    assert!(fuzz.standalone_replay.is_some());
    assert!(!fuzz.routed_to_semantic_engine);
    let suites: Vec<OracleSuiteCase> = fixture("suites.json");
    assert_eq!(suites.len(), 4);
    assert!(suites.iter().all(|case| case.retain_failure_bundle));
}

#[test]
fn all_six_pinned_chaos_families_remain_importable() {
    for name in [
        "smoke.yaml",
        "cost.yaml",
        "mixed-transport.yaml",
        "congestion.yaml",
        "churn.yaml",
        "bloom-storm.yaml",
    ] {
        let result = import_chaos_yaml(name, &common::fixture(name)).unwrap();
        assert_eq!(result.source.fips_commit, PINNED_FIPS_COMMIT);
        assert_eq!(result.plan.campaign_sha256.len(), 64);
    }
}

#[test]
fn checked_live_summary_is_honest_about_observation_limits_and_provenance() {
    let summary: Value = serde_json::from_slice(
        &fs::read(common::repository().join("fixtures/m5/live-smoke-summary.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(summary["live_verification"], true);
    assert_eq!(summary["fips_commit"], PINNED_FIPS_COMMIT);
    assert_eq!(summary["checkout_dirty"], false);
    assert_eq!(summary["repeats"], 3);
    assert_eq!(summary["successful_exits"], 3);
    assert_eq!(summary["dominant_confidence_ppm"], 1_000_000);
    assert_eq!(summary["comparative_claim_allowed"], false);
    assert_eq!(summary["unobserved_values_preserved"], true);
    assert_eq!(summary["image_ids"].as_array().unwrap().len(), 3);
    assert_eq!(summary["evidence_sha256"].as_array().unwrap().len(), 3);
}

fn generated() -> std::path::PathBuf {
    common::repository().join("fixtures/m5/generated")
}

fn fixture<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_slice(&fs::read(generated().join(Path::new(name))).unwrap()).unwrap()
}
