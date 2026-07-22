mod common;

use fips_scale::{
    BASELINE_VARIANT, BLOOM_DELTA_VARIANT, CohortEngine, DAMPENING_VARIANT, cohort_bloom,
    compare_variants, resolve_variant,
};

#[test]
fn cohort_mass_bounds_and_shared_artifact_projection_are_explicit() {
    let plan = common::billion_plan();
    let run = CohortEngine.run(&plan, BASELINE_VARIANT).unwrap();
    assert_eq!(run.report.represented_nodes, 1_000_000_000);
    assert_eq!(run.report.population_before, run.report.population_after);
    assert!(run.report.allocated_cohorts <= 64);
    assert_eq!(
        run.report
            .cohorts
            .iter()
            .map(|cohort| cohort.population)
            .sum::<u64>(),
        1_000_000_000
    );
    for estimate in [
        &run.report.metrics.maximum_depth,
        &run.report.metrics.control_bytes,
        &run.report.metrics.bloom_fpr_ppb,
        &run.report.metrics.quiescence_ns,
    ] {
        assert!(!estimate.method.is_empty());
        assert!(!estimate.uncertainty.is_empty());
        assert!(!estimate.validated_range.is_empty());
        assert!(estimate.lower.parse::<u128>().unwrap() <= estimate.value.parse().unwrap());
        assert!(estimate.value.parse::<u128>().unwrap() <= estimate.upper.parse().unwrap());
    }
    run.artifact.validate().unwrap();
    assert_eq!(run.artifact.metric_series.len(), 8);
    assert!(
        run.artifact
            .manifest
            .fidelity
            .plain_language_statement()
            .contains("analytical cohorts")
    );
}

#[test]
fn cohort_bloom_matches_closed_form_and_never_allocates_population_bits() {
    let report = CohortEngine
        .run(&common::billion_plan(), BASELINE_VARIANT)
        .unwrap()
        .report;
    let blooms = cohort_bloom(&report.cohorts, 4096, 3);
    assert_eq!(blooms.len(), report.cohorts.len());
    assert!(
        blooms
            .iter()
            .all(|bloom| bloom.occupancy_ppb <= 1_000_000_000)
    );
    assert!(
        blooms
            .iter()
            .all(|bloom| bloom.fpr_ppb <= bloom.occupancy_ppb)
    );
    let first = &blooms[0];
    let occupancy = 1.0 - (-(3.0 * first.insertions_per_member as f64 / 4096.0)).exp();
    assert_eq!(
        first.occupancy_ppb,
        (occupancy * 1_000_000_000.0).round() as u64
    );
}

#[test]
fn versioned_variants_share_one_engine_and_attribute_divergence() {
    let comparison = compare_variants(&common::billion_plan()).unwrap();
    assert_eq!(comparison.baseline.variant.id, BASELINE_VARIANT);
    assert_eq!(comparison.candidates.len(), 2);
    assert_eq!(comparison.divergences.len(), 2);
    assert!(
        comparison
            .candidates
            .iter()
            .all(|candidate| candidate.variant.experimental)
    );
    assert!(comparison.divergences.iter().any(|item| {
        item.variant == DAMPENING_VARIANT && item.attributed_to.contains("root eligibility")
    }));
    assert!(comparison.divergences.iter().any(|item| {
        item.variant == BLOOM_DELTA_VARIANT && item.attributed_to.contains("Bloom")
    }));
    assert!(
        comparison
            .experimental_notice
            .contains("not upstream recommendations")
    );
    let parameters = serde_json::json!({"x": 1});
    let first = resolve_variant(BASELINE_VARIANT, &parameters)
        .unwrap()
        .identity();
    let second = resolve_variant(BASELINE_VARIANT, &parameters)
        .unwrap()
        .identity();
    assert_eq!(first, second);
    let hooks = resolve_variant(DAMPENING_VARIANT, &parameters)
        .unwrap()
        .hooks();
    assert_eq!(hooks.root_eligibility, "tenure-gated");
    assert!(!hooks.mixed_version_support);
    assert!(!hooks.parent_choice.is_empty() && !hooks.lookup.is_empty());
    let error = match resolve_variant("baseline+delta", &parameters) {
        Ok(_) => panic!("mixed variant unexpectedly resolved"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("mixed-version"));
}
