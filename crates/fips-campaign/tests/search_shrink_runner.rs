mod common;

use fips_campaign::{
    CampaignPlanner, CampaignRunner, CancellationToken, ExecutionBudgets, HierarchicalShrinker,
    MetricConstraint, ObjectiveDirection, ObjectiveSpec, PlannerRequest, PlanningMode,
    SearchEngine, SearchRequest, ShrinkDimension,
};
use fips_engine::IndividualEngine;

fn manifest() -> fips_campaign::PlanManifest {
    CampaignPlanner::default()
        .plan(
            &common::search_plan(),
            PlannerRequest {
                mode: PlanningMode::Covering,
                strength: 2,
                ..PlannerRequest::default()
            },
        )
        .unwrap()
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
        maximum_attacker_operations: Some(10),
    }
}

#[test]
fn search_resumes_without_changing_evaluations_and_keeps_full_evidence() {
    let full = manifest();
    assert_eq!(full.cases.len(), 6);
    let mut first_half = full.clone();
    first_half.cases.truncate(3);
    let request = search_request(6);
    let first = SearchEngine
        .search(&first_half, request.clone(), None)
        .unwrap();
    let immutable = first.checkpoint.evaluated.clone();
    let resumed = SearchEngine
        .search(&full, request, Some(first.checkpoint))
        .unwrap();
    assert_eq!(resumed.checkpoint.evaluated.len(), 6);
    assert!(resumed.checkpoint.complete);
    for (id, evaluation) in immutable {
        assert_eq!(&resumed.checkpoint.evaluated[&id], &evaluation);
    }
    assert!(!resumed.best_case_ids.is_empty());
    assert!(!resumed.pareto_case_ids.is_empty());
    for id in &resumed.best_case_ids {
        let best = &resumed.checkpoint.evaluated[id];
        assert!(best.protocol_valid);
        assert!(best.artifact.is_some());
        assert!(best.reproduction.is_some());
        assert!(best.artifact_sha256.is_some());
    }
}

#[test]
fn worker_count_resume_and_budgets_do_not_change_case_outcomes() {
    let manifest = manifest();
    let budgets = |workers, maximum_cases| ExecutionBudgets {
        worker_count: workers,
        maximum_cases,
        maximum_memory_bytes: u64::MAX,
        maximum_disk_bytes: u64::MAX,
    };
    let single = CampaignRunner
        .run(
            &manifest.campaign_sha256,
            &manifest.cases,
            budgets(1, 6),
            None,
            CancellationToken::default(),
        )
        .unwrap();
    let parallel = CampaignRunner
        .run(
            &manifest.campaign_sha256,
            &manifest.cases,
            budgets(4, 6),
            None,
            CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(single.checkpoint, parallel.checkpoint);

    let partial = CampaignRunner
        .run(
            &manifest.campaign_sha256,
            &manifest.cases,
            budgets(2, 3),
            None,
            CancellationToken::default(),
        )
        .unwrap();
    assert!(partial.partial);
    assert_eq!(partial.termination.as_deref(), Some("case-count-budget"));
    let resumed = CampaignRunner
        .run(
            &manifest.campaign_sha256,
            &manifest.cases,
            budgets(3, 6),
            Some(partial.checkpoint),
            CancellationToken::default(),
        )
        .unwrap();
    assert!(!resumed.partial);
    assert_eq!(resumed.skipped_completed_cases, 3);
    assert_eq!(resumed.checkpoint, single.checkpoint);

    let disk_limited = CampaignRunner
        .run(
            &manifest.campaign_sha256,
            &manifest.cases,
            ExecutionBudgets {
                maximum_disk_bytes: 1,
                ..budgets(2, 6)
            },
            None,
            CancellationToken::default(),
        )
        .unwrap();
    assert!(disk_limited.partial);
    assert_eq!(disk_limited.termination.as_deref(), Some("disk-budget"));
}

#[test]
fn hierarchical_shrinking_preserves_metric_and_replays_final_bundle() {
    let manifest = manifest();
    let search = SearchEngine
        .search(&manifest, search_request(6), None)
        .unwrap();
    let best = &search.checkpoint.evaluated[&search.best_case_ids[0]];
    let initial = best.reproduction.as_ref().unwrap();
    let threshold = best.metrics["amplification-ppm"] * 9 / 10;
    let plan: fips_model::NormalizedPlan =
        serde_json::from_value(initial.normalized_plan.clone()).unwrap();
    let result = HierarchicalShrinker { worker_count: 4 }
        .shrink(
            initial.bundle_id.clone(),
            plan,
            (
                "amplification-threshold",
                move |candidate: &fips_model::NormalizedPlan| {
                    IndividualEngine.run_plan(candidate).is_ok_and(|run| {
                        run.recovery_report
                            .is_some_and(|report| report.costs.amplification_ppm >= threshold)
                    })
                },
            ),
        )
        .unwrap();
    assert!(result.steps.iter().any(|step| step.predicate_held));
    assert!(
        result
            .steps
            .iter()
            .any(|step| step.dimension == ShrinkDimension::Traffic)
    );
    let bundle = result.reproduction.as_ref().unwrap();
    let final_plan: fips_model::NormalizedPlan =
        serde_json::from_value(bundle.normalized_plan.clone()).unwrap();
    let replay = IndividualEngine.run_plan(&final_plan).unwrap();
    assert!(replay.recovery_report.unwrap().costs.amplification_ppm >= threshold);
}

#[test]
fn symbolic_million_node_failure_reduces_without_materializing_population() {
    let mut plan = common::search_plan();
    *plan.campaign.pointer_mut("/scale/nodes").unwrap() = serde_json::json!(1_000_000);
    plan.axes.clear();
    plan.campaign_sha256 = "synthetic-million".to_owned();
    let result = HierarchicalShrinker { worker_count: 4 }
        .shrink(
            "synthetic-million",
            plan,
            (
                "at-least-16-nodes",
                |candidate: &fips_model::NormalizedPlan| {
                    candidate
                        .campaign
                        .pointer("/scale/nodes")
                        .and_then(serde_json::Value::as_u64)
                        .is_some_and(|nodes| nodes >= 16)
                },
            ),
        )
        .unwrap();
    let nodes = result
        .final_plan
        .campaign
        .pointer("/scale/nodes")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert!((16..1_000_000).contains(&nodes));
    assert!(
        result
            .steps
            .iter()
            .any(|step| { step.dimension == ShrinkDimension::Nodes && step.predicate_held })
    );
}
