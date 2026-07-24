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
fn streaming_observer_matches_persisted_total_order() {
    let plan = campaign("500ms", false);
    let mut streamed = Vec::new();
    let run = IndividualEngine
        .run_plan_streaming(&plan, &mut |event| {
            streamed.push(event.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(streamed, run.artifact.event_trace);
}

#[test]
fn streaming_observer_failure_stops_the_run() {
    let error = IndividualEngine
        .run_plan_streaming(&campaign("500ms", false), &mut |_| {
            Err("consumer disconnected".to_owned())
        })
        .unwrap_err();
    assert!(error.to_string().contains("consumer disconnected"));
}

#[test]
fn initial_event_contains_renderable_topology_truth() {
    let run = IndividualEngine
        .run_plan(&campaign("500ms", false))
        .unwrap();
    let initial = run.artifact.event_trace.first().unwrap();
    assert_eq!(initial.kind, "input.initial-topology");
    assert_eq!(initial.data["nodes"].as_array().unwrap().len(), 8);
    assert_eq!(initial.data["edges"].as_array().unwrap().len(), 7);
    assert_eq!(initial.data["active_nodes"], 6);
    assert_eq!(initial.data["nodes"][0]["id"], 0);
    assert_eq!(initial.data["edges"][0]["from"], 0);
    assert_eq!(initial.data["edges"][0]["to"], 1);
}

#[test]
fn quiescent_assertion_failure_remains_a_replayable_artifact() {
    let mut plan = campaign("1s", false);
    *plan.campaign.pointer_mut("/scale/nodes").unwrap() = json!(1_000);
    *plan
        .campaign
        .pointer_mut("/identities/arrivals/count")
        .unwrap() = json!(8);
    *plan
        .campaign
        .pointer_mut("/identities/arrivals/attacker_budget/operations")
        .unwrap() = json!(8);
    *plan.campaign.pointer_mut("/topology/generator").unwrap() = json!("random-regular");
    *plan
        .campaign
        .pointer_mut("/topology/average_degree")
        .unwrap() = json!(4);
    *plan
        .campaign
        .pointer_mut("/identities/arrivals/attachment")
        .unwrap() = json!("random");
    *plan.campaign.pointer_mut("/links/latency").unwrap() = json!({"nanoseconds": 20_000_000_u64});
    *plan.campaign.pointer_mut("/links/bandwidth_bps").unwrap() = json!(100_000_000);
    *plan.campaign.pointer_mut("/links/ordering").unwrap() = json!("datagram");
    *plan.campaign.pointer_mut("/links/mtu_bytes").unwrap() = json!(1_500);
    plan.seed = 424_242;
    *plan.campaign.pointer_mut("/seed").unwrap() = json!(424_242);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    assert!(run.artifact.validate().is_ok());
    assert!(
        run.report
            .assertions
            .iter()
            .any(|assertion| assertion.outcome == "fail")
    );
    assert_eq!(
        run.artifact.assertion_results, run.report.assertions,
        "the artifact must retain the scientific failure"
    );
}

#[test]
fn mixed_node_transports_drive_rendered_profiles_and_edge_bottlenecks() {
    let mut plan = campaign("500ms", false);
    *plan.campaign.pointer_mut("/transports").unwrap() = json!({
        "assignment": "random-mixed",
        "profiles": [
            {
                "name": "wifi", "type": "wifi", "mtu_bytes": 1500,
                "latency": {"nanoseconds": 8_000_000_u64},
                "bandwidth_bps": 100_000_000_u64, "weight": 0
            },
            {
                "name": "bluetooth", "type": "ble", "mtu_bytes": 244,
                "latency": {"nanoseconds": 20_000_000_u64},
                "bandwidth_bps": 1_000_000_u64, "weight": 1
            }
        ]
    });
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let initial = &run.artifact.event_trace[0].data;
    assert!(initial["nodes"].as_array().unwrap().iter().all(|node| {
        node["transport_profile"] == "bluetooth" && node["bandwidth_bps"] == 1_000_000_u64
    }));
    let due = run
        .artifact
        .event_trace
        .iter()
        .find(|event| {
            event.kind == "tree-announce.due" && event.data.get("bandwidth_bps").is_some()
        })
        .unwrap();
    assert_eq!(due.data["bandwidth_bps"], 1_000_000_u64);
    assert_eq!(due.data["mtu_bytes"], 244);
    assert_eq!(due.data["from_transport"], "bluetooth");
}

#[test]
fn authored_media_zone_overrides_edges_and_is_visible_in_topology() {
    let mut plan = campaign("500ms", false);
    plan.campaign["topology"]["media_zones"] = json!([{
        "id": "lab-wifi", "nodes": [0, 1, 2, 3, 4, 5],
        "bandwidth_bps": 2_000_000_u64,
        "latency": {"nanoseconds": 40_000_000_u64},
        "loss_ppm": 10_000, "mtu_bytes": 1200, "queue_bytes": 4096
    }]);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let initial = &run.artifact.event_trace[0].data;
    assert!(
        initial["nodes"].as_array().unwrap()[..6]
            .iter()
            .all(|node| node["media_zone"] == "lab-wifi")
    );
    assert!(
        initial["edges"].as_array().unwrap()[..5]
            .iter()
            .all(|edge| {
                edge["shared_medium_group"] == 0
                    && edge["bandwidth_bps"] == 2_000_000_u64
                    && edge["mtu_bytes"] == 1200
            })
    );
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
fn irregular_manual_lower_root_arrival_is_reserved_and_replayable() {
    let mut plan = campaign("500ms", false);
    *plan.campaign.pointer_mut("/events").unwrap() = json!([
        {"id": "manual-root", "at": {"nanoseconds": 2_250_000_000_u64}, "action": "introduce-lower-root-node"}
    ]);
    *plan
        .campaign
        .pointer_mut("/identities/arrivals/attacker_budget/operations")
        .unwrap() = json!(3);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let manual = run
        .artifact
        .event_trace
        .iter()
        .find(|event| {
            event.kind == "input.descending-root-arrival" && event.virtual_time_ns == 2_250_000_000
        })
        .unwrap();
    assert_eq!(manual.data["node"], 7);
    assert_eq!(run.report.arrivals, 3);
}

#[test]
fn partition_and_merge_change_exact_edges_then_reconverge() {
    let mut plan = campaign("500ms", false);
    *plan.campaign.pointer_mut("/events").unwrap() = json!([
        {"id": "split", "at": {"nanoseconds": 3_000_000_000_u64}, "action": "partition-network", "parameters": {"nodes": [0, 1, 2, 3]}},
        {"id": "heal", "at": {"nanoseconds": 4_000_000_000_u64}, "action": "merge-network", "parameters": {"nodes": [0, 1, 2, 3]}}
    ]);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let split = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.network-partitioned")
        .unwrap();
    let heal = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.network-merged")
        .unwrap();
    assert!(!split.data["changed_edges"].as_array().unwrap().is_empty());
    assert_eq!(split.data["changed_edges"], heal.data["changed_edges"]);
    assert!(
        run.report
            .assertions
            .iter()
            .all(|item| item.outcome == "pass")
    );
}

#[test]
fn link_conditions_change_and_restore_without_resetting_edge_identity() {
    let mut plan = campaign("500ms", false);
    *plan.campaign.pointer_mut("/events").unwrap() = json!([
        {"id": "slow", "at": {"nanoseconds": 750_000_000_u64}, "action": "set-link-conditions", "target": 0, "parameters": {"bandwidth_bps": 1000, "latency": {"nanoseconds": 90_000_000_u64}, "mtu_bytes": 512}},
        {"id": "restore", "at": {"nanoseconds": 1_750_000_000_u64}, "action": "restore-link-conditions", "target": 0}
    ]);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let changes = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind.starts_with("input.link-conditions"))
        .collect::<Vec<_>>();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].data["edge"], 0);
    assert_eq!(changes[0].data["after"]["bandwidth_bps"], 1000);
    assert_eq!(changes[1].data["after"]["bandwidth_bps"], 1_000_000_000_u64);
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
