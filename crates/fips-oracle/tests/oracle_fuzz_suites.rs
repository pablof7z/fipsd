mod common;

use fips_oracle::{
    ComparableEvidence, ComparableTransition, DaemonEvidence, FuzzOutcome, NormalizedTelemetry,
    OracleClassification, RecordedBackend, TELEMETRY_ADAPTER_VERSION, TelemetryInput,
    adapt_fuzz_result, default_oracle_suites, fixture_provenance, import_chaos_yaml,
    normalize_telemetry, run_oracle,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn repeated_oracle_distinguishes_stable_reproduction_from_environmental_flake() {
    let plan = import_chaos_yaml("smoke.yaml", &common::fixture("smoke.yaml"))
        .unwrap()
        .plan;
    let model = trace("root-a");
    let bad = daemon("root-b");
    let stable = RecordedBackend {
        id: "recorded-real-daemon".to_owned(),
        evidence: vec![bad.clone()],
    };
    let report = run_oracle(&plan, &model, &stable, 3, 800_000).unwrap();
    assert!(report.stable && report.oracle_predicate_held);
    assert_eq!(
        report.dominant_classification,
        OracleClassification::ImplementationBug
    );
    assert_eq!(report.attached_daemon_evidence.len(), 3);

    let flaky = RecordedBackend {
        id: "recorded-flaky-host".to_owned(),
        evidence: vec![bad, daemon("root-a"), daemon("root-a")],
    };
    let report = run_oracle(&plan, &model, &flaky, 3, 800_000).unwrap();
    assert!(!report.stable);
    assert!(!report.oracle_predicate_held);
    assert_eq!(report.dominant_confidence_ppm, 666_666);
}

#[test]
fn invalid_wire_results_are_standalone_and_never_enter_semantic_execution() {
    let crash = adapt_fuzz_result(
        "cargo-fuzz",
        FuzzOutcome::Crash,
        &[0xff, 0x00],
        "corpus-sha",
        42,
    );
    assert_eq!(crash.codec_commit, fips_oracle::PINNED_FIPS_COMMIT);
    assert_eq!(crash.minimized_input_hex.as_deref(), Some("ff00"));
    assert!(crash.standalone_replay.is_some());
    assert!(!crash.routed_to_semantic_engine);
    let pass = adapt_fuzz_result("libafl", FuzzOutcome::Pass, &[], "corpus-sha", 99);
    assert!(pass.minimized_input_hex.is_none());
}

#[test]
fn oracle_suites_pin_budgets_revisions_cache_keys_and_failure_retention() {
    let suites = default_oracle_suites();
    assert_eq!(suites.len(), 4);
    assert!(suites.iter().all(|case| case.maximum_minutes <= 60));
    assert!(
        suites
            .iter()
            .all(|case| !case.revision.is_empty() && !case.cache_key.is_empty())
    );
    assert!(suites.iter().all(|case| case.retain_failure_bundle));
    assert!(
        suites
            .iter()
            .any(|case| case.expected == "implementation-bug")
    );
}

#[test]
fn empty_recorded_backend_returns_a_typed_error_instead_of_panicking() {
    let plan = import_chaos_yaml("smoke.yaml", &common::fixture("smoke.yaml"))
        .unwrap()
        .plan;
    let backend = RecordedBackend {
        id: "empty".to_owned(),
        evidence: Vec::new(),
    };
    let error = run_oracle(&plan, &trace("root-a"), &backend, 1, 800_000).unwrap_err();
    assert!(error.to_string().contains("no recorded evidence"));
}

fn trace(root: &str) -> ComparableEvidence {
    ComparableEvidence {
        transitions: vec![ComparableTransition {
            ordinal: 0,
            kind: "root".to_owned(),
            node: "n01".to_owned(),
            state: root.to_owned(),
            at_ns: 0,
            evidence: format!("tree:{root}"),
        }],
        frames: Vec::new(),
        metrics: BTreeMap::new(),
        unsupported_fields: BTreeSet::new(),
    }
}

fn daemon(root: &str) -> DaemonEvidence {
    DaemonEvidence {
        kind: "recorded-real-daemon-evidence/v1alpha1".to_owned(),
        comparable: trace(root),
        telemetry: empty_telemetry(),
        provenance: fixture_provenance(
            b"real-fips-binary",
            "sha256:recorded-image",
            &BTreeMap::new(),
        ),
        raw_output_sha256: format!("output-{root}"),
        exit_code: 0,
    }
}

fn empty_telemetry() -> NormalizedTelemetry {
    normalize_telemetry(TelemetryInput {
        adapter_version: TELEMETRY_ADAPTER_VERSION.to_owned(),
        sources: Vec::new(),
        clock_offset_ns: 0,
        clock_uncertainty_ns: 1_000_000,
    })
    .unwrap()
}
