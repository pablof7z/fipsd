use super::*;
use fips_model::normalize_str;

fn campaign(interval: &str, broken: bool) -> NormalizedPlan {
    let fault = if broken {
        "events: [{id: break, at: 1s, action: inject-parent-loop}]"
    } else {
        "events: [{id: arrivals, action: introduce-lower-root-identities}]"
    };
    normalize_str(&format!(
            r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {{name: m1-test}}
seed: 7
engine: {{modes: compact-discrete-event, deterministic: true}}
scale: {{nodes: 8}}
topology: {{generator: chain, average_degree: 2}}
identities:
  initial: {{distribution: uniform-128}}
  arrivals:
    count: 2
    schedule: {{start: 1s, interval: {interval}}}
    address_policy: strictly-lower-than-current-root
    attachment: current-root
    attacker_budget: {{mode: bounded, operations: 2}}
transports: {{assignment: all-udp}}
links: {{latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, ordering: stream, mtu_bytes: 9000, queue_bytes: 1048576}}
resources: {{assignment: uniform}}
{fault}
protocol: {{variant: fips-80c956a-baseline, parameters: {{tree_announce_debounce: 500ms}}}}
traffic: {{model: idle}}
fidelity: {{protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}}
accounting: {{causal_lineage: true, reconcile_serialized_frames: true}}
instrumentation: {{transition_stages: true}}
assertions: []
objectives: {{maximize: [control-bytes]}}
"#
        ))
        .unwrap()
}

#[test]
fn same_seed_is_byte_identical_and_invariants_pass() {
    let plan = campaign("500ms", false);
    let left = IndividualEngine.run_plan(&plan).unwrap();
    let right = IndividualEngine.run_plan(&plan).unwrap();
    assert_eq!(
        left.artifact.to_canonical_json().unwrap(),
        right.artifact.to_canonical_json().unwrap()
    );
    assert!(
        left.report
            .assertions
            .iter()
            .all(|assertion| assertion.outcome == "pass")
    );
    assert_eq!(left.report.final_root, left.report.root_generations[0]);
}

#[test]
fn debounce_boundaries_are_enforced() {
    for interval in ["499ms", "500ms", "501ms"] {
        let run = IndividualEngine
            .run_plan(&campaign(interval, false))
            .unwrap();
        assert!(
            run.report
                .assertions
                .iter()
                .find(|assertion| assertion.id == "per-peer-debounce")
                .is_some_and(|assertion| assertion.outcome == "pass")
        );
    }
}

#[test]
fn deliberately_broken_fixture_fails_loud() {
    let error = IndividualEngine
        .run_plan(&campaign("500ms", true))
        .unwrap_err();
    assert!(error.to_string().contains("loop-freedom"));
}

#[test]
fn bounded_identity_generation_exhausts_deterministically() {
    let mut plan = campaign("500ms", false);
    *plan
        .campaign
        .pointer_mut("/identities/arrivals/attacker_budget/operations")
        .unwrap() = Value::from(1);
    assert!(matches!(
        IndividualEngine.run_plan(&plan),
        Err(RunError::BudgetExhausted { .. })
    ));
}

#[test]
fn precomputed_ladder_is_validated_and_charges_no_grinding_trials() {
    let mut plan = campaign("500ms", false);
    let arrivals = plan
        .campaign
        .pointer_mut("/identities/arrivals")
        .and_then(Value::as_object_mut)
        .unwrap();
    arrivals.insert(
        "address_policy".to_owned(),
        Value::String("precomputed-ladder".to_owned()),
    );
    arrivals.insert(
        "precomputed_ladder".to_owned(),
        json!([
            "7fffffffffffffffffffffffffffff00",
            "7ffffffffffffffffffffffffffffe00"
        ]),
    );
    *plan
        .campaign
        .pointer_mut("/identities/arrivals/attacker_budget/operations")
        .unwrap() = Value::from(0);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    assert_eq!(run.report.identity_generation_trials, 0);
    assert_eq!(run.report.final_root, "7ffffffffffffffffffffffffffffe00");
}

#[test]
fn disappearance_and_reappearance_reconverge() {
    let mut plan = campaign("500ms", false);
    *plan.campaign.pointer_mut("/events").unwrap() = json!([
        {"id": "down", "at": {"nanoseconds": 2_000_000_000_u64}, "action": "disappear-node", "target": 7},
        {"id": "up", "at": {"nanoseconds": 3_000_000_000_u64}, "action": "reappear-node", "target": 7}
    ]);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    assert!(
        run.artifact
            .event_trace
            .iter()
            .any(|event| event.kind == "input.node-disappeared")
    );
    assert!(
        run.artifact
            .event_trace
            .iter()
            .any(|event| event.kind == "input.node-reappeared")
    );
    assert!(
        run.report
            .assertions
            .iter()
            .all(|assertion| assertion.outcome == "pass")
    );
}

#[test]
fn better_root_bypasses_parent_hold_down() {
    let mut plan = campaign("500ms", false);
    let parameters = plan
        .campaign
        .pointer_mut("/protocol/parameters")
        .and_then(Value::as_object_mut)
        .unwrap();
    parameters.insert(
        "parent_hold_down".to_owned(),
        json!({"nanoseconds": 10_000_000_000_u64}),
    );
    let run = IndividualEngine.run_plan(&plan).unwrap();
    assert_eq!(run.report.final_root, run.report.root_generations[0]);
    assert!(
        run.report
            .assertions
            .iter()
            .find(|assertion| assertion.id == "root-agreement")
            .is_some_and(|assertion| assertion.outcome == "pass")
    );
}

#[test]
fn current_fips_pin_is_exact() {
    assert_eq!(FIPS_COMMIT, fips_artifact::M0_FIPS_COMMIT);
}
