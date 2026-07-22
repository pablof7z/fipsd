mod common;

use fips_campaign::{
    DaemonConfirmation, ShrinkResult, corpus_entry, load_corpus, promote, replay_corpus,
};
use fips_engine::IndividualEngine;

#[test]
fn promotion_replay_and_expectation_changes_are_explicit() {
    let plan = common::recovery_plan();
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let shrink = ShrinkResult {
        kind: "hierarchical-shrink/v1alpha1".to_owned(),
        source_case_id: "case-source".to_owned(),
        predicate: "fixture".to_owned(),
        initial_plan_sha256: plan.campaign_sha256.clone(),
        final_plan: plan,
        steps: Vec::new(),
        cache_entries: 1,
        reproduction: Some(run.reproduction),
    };
    let entry = corpus_entry(
        &shrink,
        "m3-test",
        ">=0.1.0,<0.2.0",
        DaemonConfirmation::ModelOnly,
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    promote(temporary.path(), &entry, false).unwrap();
    let loaded = load_corpus(temporary.path()).unwrap();
    assert_eq!(loaded, vec![entry.clone()]);
    let report = replay_corpus(temporary.path()).unwrap();
    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.outcomes[0].first_seen, "m3-test");
    assert_eq!(report.outcomes[0].minimized_from, "case-source");

    let mut changed = entry;
    changed
        .metadata
        .expected_assertions
        .push("review-required".to_owned());
    assert!(
        promote(temporary.path(), &changed, false)
            .unwrap_err()
            .to_string()
            .contains("explicit reviewed update")
    );
    promote(temporary.path(), &changed, true).unwrap();
}
