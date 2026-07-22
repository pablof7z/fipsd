mod common;

use fips_campaign::{
    CampaignPlanner, DaemonConfirmation, HierarchicalShrinker, MetricConstraint,
    ObjectiveDirection, ObjectiveSpec, PlannerRequest, PlanningMode, SearchEngine, SearchRequest,
    load_corpus, replay_corpus,
};
use fips_engine::IndividualEngine;
use std::fs;

#[test]
fn checked_search_shrink_and_corpus_evidence_replays() {
    let root = common::repository();
    let source = common::search_plan();
    let manifest = CampaignPlanner::default()
        .plan(
            &source,
            PlannerRequest {
                mode: PlanningMode::Covering,
                strength: 2,
                seed: source.seed,
                ..PlannerRequest::default()
            },
        )
        .unwrap();
    let expected_manifest: fips_campaign::PlanManifest =
        serde_json::from_slice(&fs::read(root.join("fixtures/m3/covering-plan.json")).unwrap())
            .unwrap();
    assert_eq!(manifest, expected_manifest);
    assert_eq!(manifest.total_combinations, 32);
    assert_eq!(manifest.cases.len(), 6);
    assert_eq!(manifest.covered_interactions, manifest.total_interactions);

    let search = SearchEngine
        .search(&manifest, search_request(manifest.cases.len()), None)
        .unwrap();
    let expected_search: fips_campaign::SearchResult =
        serde_json::from_slice(&fs::read(root.join("fixtures/m3/search-result.json")).unwrap())
            .unwrap();
    assert_eq!(search, expected_search);
    let best = &search.checkpoint.evaluated[&search.best_case_ids[0]];
    assert!(best.artifact.is_some() && best.reproduction.is_some());

    let expected_shrink: fips_campaign::ShrinkResult =
        serde_json::from_slice(&fs::read(root.join("fixtures/m3/shrink-result.json")).unwrap())
            .unwrap();
    let initial = best.reproduction.as_ref().unwrap();
    let threshold = best.metrics["amplification-ppm"] * 9 / 10;
    let plan = serde_json::from_value(initial.normalized_plan.clone()).unwrap();
    let shrink = HierarchicalShrinker { worker_count: 4 }
        .shrink(
            initial.bundle_id.clone(),
            plan,
            (
                "amplification-at-least-90-percent",
                move |candidate: &fips_model::NormalizedPlan| {
                    IndividualEngine.run_plan(candidate).is_ok_and(|run| {
                        run.recovery_report
                            .is_some_and(|report| report.costs.amplification_ppm >= threshold)
                    })
                },
            ),
        )
        .unwrap();
    assert_eq!(shrink, expected_shrink);
    let bundle = shrink.reproduction.unwrap();
    let replay_plan = serde_json::from_value(bundle.normalized_plan).unwrap();
    assert!(IndividualEngine.run_plan(&replay_plan).is_ok());

    let corpus = load_corpus(&root.join("fixtures/corpus")).unwrap();
    assert_eq!(corpus.len(), 1);
    assert_eq!(corpus[0].id, "m3-root-ratchet");
    assert_eq!(
        corpus[0].metadata.daemon_confirmation,
        DaemonConfirmation::ModelOnly
    );
    let report = replay_corpus(&root.join("fixtures/corpus")).unwrap();
    assert_eq!((report.passed, report.failed), (1, 0));
}

fn search_request(maximum_evaluations: usize) -> SearchRequest {
    SearchRequest {
        objectives: vec![
            ObjectiveSpec {
                metric: "amplification-ppm".to_owned(),
                direction: ObjectiveDirection::Maximize,
            },
            ObjectiveSpec {
                metric: "goodput-stall-ns".to_owned(),
                direction: ObjectiveDirection::Maximize,
            },
        ],
        constraints: vec![MetricConstraint {
            metric: "starved-flows".to_owned(),
            maximum: None,
            minimum: Some(0),
        }],
        maximum_evaluations,
        maximum_attacker_operations: Some(1_000_000),
    }
}
