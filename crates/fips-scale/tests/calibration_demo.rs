mod common;

use fips_scale::{
    BASELINE_VARIANT, BLOOM_DELTA_VARIANT, DAMPENING_VARIANT, billion_node_demo, calibrate,
};
use std::collections::BTreeSet;
use std::time::Instant;

#[test]
fn calibration_publishes_error_distributions_ranges_and_machine_warnings() {
    let report = calibrate(&common::billion_plan()).unwrap();
    assert_eq!(report.samples.len(), 56);
    assert!(report.hybrid_sample_replayed);
    assert_eq!(report.distributions.len(), 7);
    for distribution in report.distributions.values() {
        assert_eq!(distribution.errors_ppm.len(), 8);
        assert!(distribution.validated_range.contains("8..=64"));
        assert!(distribution.maximum_absolute_ppm >= distribution.median_absolute_ppm);
        assert_eq!(
            distribution.warning.is_some(),
            distribution.maximum_absolute_ppm > 250_000
        );
    }
}

#[test]
fn billion_demo_is_bounded_sensitive_and_contains_one_exact_anomaly() {
    let started = Instant::now();
    let demo = billion_node_demo(&common::billion_plan()).unwrap();
    assert_eq!(demo.represented_nodes, 1_000_000_000);
    assert_eq!(demo.scenarios.len(), 18);
    assert!(
        demo.scenarios
            .iter()
            .all(|scenario| scenario.allocated_cohorts <= 64)
    );
    assert_eq!(demo.exact_anomaly.exact_population, 16);
    assert!(demo.headline_warning.contains("No individual-node claim"));
    assert!(
        demo.minimum_control_bytes.parse::<u128>().unwrap()
            < demo.maximum_control_bytes.parse().unwrap()
    );
    let variants = demo
        .scenarios
        .iter()
        .map(|scenario| scenario.variant.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        variants,
        BTreeSet::from([BASELINE_VARIANT, DAMPENING_VARIANT, BLOOM_DELTA_VARIANT])
    );
    let topologies = demo
        .scenarios
        .iter()
        .map(|scenario| scenario.topology.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(topologies, BTreeSet::from(["balanced-tree", "chain"]));
    assert!(started.elapsed().as_secs() < demo.resource_budget.maximum_wall_time_seconds);
    assert!(demo.resource_budget.bounded_allocation);
}
