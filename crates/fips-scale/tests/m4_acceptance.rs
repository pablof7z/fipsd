mod common;

use fips_scale::{
    BASELINE_VARIANT, BillionNodeDemo, CalibrationReport, CohortEngine, CohortReport, HybridEngine,
    HybridReport, SamplingPolicy, VariantComparison, billion_node_demo, calibrate,
    compare_variants,
};
use std::fs;

#[test]
fn checked_m4_evidence_is_reproducible_and_replayable() {
    let root = common::repository();
    let plan = common::billion_plan();
    let cohort = CohortEngine.run(&plan, BASELINE_VARIANT).unwrap();
    let expected: CohortReport = read(&root.join("fixtures/m4/billion-cohort-report.json"));
    assert_eq!(cohort.report, expected);
    let artifact = fs::read(root.join("fixtures/m4/billion-cohort-artifact.json")).unwrap();
    assert_eq!(cohort.artifact.to_canonical_json().unwrap(), artifact);

    let comparison = compare_variants(&plan).unwrap();
    let expected: VariantComparison = read(&root.join("fixtures/m4/variant-comparison.json"));
    assert_eq!(comparison, expected);

    let hybrid = HybridEngine
        .run(&plan, BASELINE_VARIANT, SamplingPolicy::AnomalyDriven, 16)
        .unwrap();
    let expected: HybridReport = read(&root.join("fixtures/m4/hybrid-anomaly.json"));
    assert_eq!(hybrid.report, expected);
    let exact_plan =
        serde_json::from_value(hybrid.report.exact.reproduction.normalized_plan).unwrap();
    assert!(fips_engine::IndividualEngine.run_plan(&exact_plan).is_ok());

    let calibration = calibrate(&plan).unwrap();
    let expected: CalibrationReport = read(&root.join("fixtures/m4/calibration.json"));
    assert_eq!(calibration, expected);
    let demo = billion_node_demo(&plan).unwrap();
    let expected: BillionNodeDemo = read(&root.join("fixtures/m4/billion-demo.json"));
    assert_eq!(demo, expected);
}

fn read<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> T {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
