mod common;

use fips_scale::{
    BASELINE_VARIANT, CalibrationProfile, CohortEngine, CryptoMode, HybridEngine, SamplingPolicy,
    account_crypto,
};
use std::collections::BTreeMap;

#[test]
fn every_sampling_policy_emits_replayable_exact_lineage_and_reconciles() {
    let plan = common::billion_plan();
    for policy in [
        SamplingPolicy::RootSpine,
        SamplingPolicy::BottleneckCut,
        SamplingPolicy::SelectedSubtree,
        SamplingPolicy::AnomalyDriven,
    ] {
        let run = HybridEngine
            .run(&plan, BASELINE_VARIANT, policy, 8)
            .unwrap();
        assert_eq!(run.report.reconciled_population, 1_000_000_000);
        assert_eq!(
            run.report.aggregate_population + run.report.exact_population,
            1_000_000_000
        );
        assert!(run.report.exact.boundary.reconciles);
        assert!(run.report.causal_transition.contains("cohort:"));
        run.report.exact.artifact.validate().unwrap();
        run.report.exact.reproduction.to_canonical_json().unwrap();
        assert_eq!(run.artifact.manifest.fidelity.sampled_regions.len(), 1);
    }
}

#[test]
fn crypto_modes_preserve_semantics_and_enforce_provenance_and_scale() {
    let cohort = CohortEngine
        .run(&common::billion_plan(), BASELINE_VARIANT)
        .unwrap()
        .report;
    let profile = CalibrationProfile {
        id: "m4-test-host".to_owned(),
        benchmark_host: "test-arm64".to_owned(),
        code_revision: "80c956a".to_owned(),
        benchmark_date: "2026-07-21".to_owned(),
        samples: 1000,
        median_ns: BTreeMap::from([("sha256".to_owned(), 120)]),
        p95_ns: BTreeMap::from([("sha256".to_owned(), 150)]),
        uncertainty: "median and p95 from 1000 samples".to_owned(),
    };
    let digest = "same-semantic-outcome";
    let counted = account_crypto(
        CryptoMode::OperationCount,
        &cohort.operation_counts,
        1_000_000_000,
        None,
        None,
        digest,
    )
    .unwrap();
    let calibrated = account_crypto(
        CryptoMode::CalibratedCost,
        &cohort.operation_counts,
        1_000_000_000,
        Some(profile.clone()),
        None,
        digest,
    )
    .unwrap();
    let unbounded = account_crypto(
        CryptoMode::Unbounded,
        &cohort.operation_counts,
        1_000_000_000,
        None,
        None,
        digest,
    )
    .unwrap();
    let budgeted = account_crypto(
        CryptoMode::AdversarialBudget,
        &cohort.operation_counts,
        1_000_000_000,
        None,
        Some(10),
        digest,
    )
    .unwrap();
    assert_eq!(
        counted.semantic_outcome_digest,
        calibrated.semantic_outcome_digest
    );
    assert_eq!(
        counted.semantic_outcome_digest,
        unbounded.semantic_outcome_digest
    );
    assert_eq!(
        counted.semantic_outcome_digest,
        budgeted.semantic_outcome_digest
    );
    assert_eq!(calibrated.profile, Some(profile));
    assert!(calibrated.calibrated_ns.is_some());
    assert!(budgeted.exhausted);
    assert!(
        account_crypto(
            CryptoMode::Execute,
            &cohort.operation_counts,
            10_001,
            None,
            None,
            digest
        )
        .is_err()
    );
    let small = account_crypto(
        CryptoMode::Execute,
        &BTreeMap::from([("sha256".to_owned(), 4)]),
        8,
        None,
        None,
        digest,
    )
    .unwrap();
    assert_eq!(small.executed_operations, 4);
}
