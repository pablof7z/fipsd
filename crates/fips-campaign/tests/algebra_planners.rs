mod common;

use fips_campaign::{
    CampaignPlanner, CaseCompiler, CompileError, CompilerLimits, PlannerRequest, PlanningMode,
};
use serde_json::json;

#[test]
fn equivalent_inputs_and_selection_order_have_identical_case_ids() {
    let plan = common::search_plan();
    let selection = plan
        .axes
        .iter()
        .map(|axis| (axis.path.clone(), axis.values[0].clone()))
        .collect();
    let left = CaseCompiler::default().compile(&plan, selection).unwrap();

    let source =
        std::fs::read_to_string(common::repository().join("examples/m3/root-ratchet-search.yaml"))
            .unwrap();
    let equivalent = fips_model::normalize_str(&format!("# formatting only\n{source}\n")).unwrap();
    let reversed = equivalent
        .axes
        .iter()
        .rev()
        .map(|axis| (axis.path.clone(), axis.values[0].clone()))
        .collect();
    let right = CaseCompiler::default()
        .compile(&equivalent, reversed)
        .unwrap();
    assert_eq!(left.case_id, right.case_id);
    assert_eq!(left.plan.campaign_sha256, right.plan.campaign_sha256);
    assert_eq!(left.derived["engine.variant"], "fips-80c956a-baseline");
    assert_eq!(left.derived["topology.connected"], true);
}

#[test]
fn constraints_and_explosive_matrices_fail_with_named_dimensions() {
    let mut plan = common::search_plan();
    plan.campaign
        .pointer_mut("/objectives/constraints")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(json!("/scale/nodes < 10"));
    let selection = plan
        .axes
        .iter()
        .map(|axis| {
            let selected = if axis.path == "/scale/nodes" {
                json!(12)
            } else {
                axis.values[0].clone()
            };
            (axis.path.clone(), selected)
        })
        .collect();
    assert!(matches!(
        CaseCompiler::default().compile(&plan, selection),
        Err(CompileError::Constraint { left, right, .. })
            if left == "/scale/nodes" && right == "10"
    ));
    assert!(matches!(
        CaseCompiler::new(CompilerLimits {
            maximum_matrix_cases: 31,
        })
        .matrix_size(&plan),
        Err(CompileError::ExplosiveMatrix {
            cases: 32,
            maximum: 31
        })
    ));
}

#[test]
fn cartesian_covering_and_monte_carlo_are_exact_and_replayable() {
    let plan = common::search_plan();
    let planner = CampaignPlanner::default();
    let cartesian = planner
        .plan(
            &plan,
            PlannerRequest {
                mode: PlanningMode::Cartesian,
                ..PlannerRequest::default()
            },
        )
        .unwrap();
    assert_eq!(cartesian.total_combinations, 32);
    assert_eq!(cartesian.cases.len(), 32);

    let covering = planner
        .plan(
            &plan,
            PlannerRequest {
                mode: PlanningMode::Covering,
                strength: 2,
                ..PlannerRequest::default()
            },
        )
        .unwrap();
    assert!(covering.cases.len() < cartesian.cases.len());
    assert_eq!(covering.covered_interactions, covering.total_interactions);
    assert_eq!(covering.total_interactions, 40);

    let request = PlannerRequest {
        mode: PlanningMode::MonteCarlo,
        run_count: 12,
        seed: 99,
        confidence_target: Some(0.95),
        stopping_criterion: Some("fixed-runs".to_owned()),
        ..PlannerRequest::default()
    };
    let first = planner.plan(&plan, request.clone()).unwrap();
    let second = planner.plan(&plan, request).unwrap();
    assert_eq!(first, second);
    assert!(!first.cases.is_empty());
    assert!(first.cases.len() <= 12);
}
