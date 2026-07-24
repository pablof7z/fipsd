use super::*;
use fips_model::normalize_str;

fn sybil_campaign(count: u64, budget: u64, policy: &str) -> NormalizedPlan {
    normalize_str(&format!(
        r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {{name: sybil-test}}
seed: 1010
engine: {{modes: compact-discrete-event, deterministic: true}}
scale: {{nodes: 10}}
topology: {{generator: random-regular, average_degree: 4}}
identities: {{initial: {{distribution: uniform-128}}}}
transports: {{assignment: all-udp}}
links: {{latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, ordering: datagram, mtu_bytes: 9000, queue_bytes: 1048576}}
resources: {{assignment: uniform}}
events:
  - id: sybil-wave
    action: attach-authenticated-sybils
    at: 2s
    parameters:
      count: {count}
      interval: 100ms
      attachment: hub
      address_policy: {policy}
      operations_per_identity: 4
adversaries:
  mode: authenticated-protocol-valid
  actions: [sybil-concentration]
  budgets: {{operations: 100, identities: {budget}, bytes: 1000000, compute_units: 1000, wall_time: 10s}}
protocol:
  variant: fips-80c956a-baseline
  parameters: {{tree_announce_debounce: 500ms, bloom_update_debounce: 500ms}}
traffic: {{model: idle}}
fidelity: {{protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}}
accounting: {{causal_lineage: true, reconcile_serialized_frames: true}}
instrumentation: {{transition_stages: true, quiescence_markers: [root, tree, bloom]}}
assertions: []
objectives: {{maximize: [sybil-concentration]}}
"#
    ))
    .unwrap()
}

#[test]
fn authenticated_sybils_join_as_individual_renderable_nodes() {
    let run = IndividualEngine
        .run_plan(&sybil_campaign(3, 3, "uniform-valid"))
        .unwrap();
    let arrivals = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "input.authenticated-sybil-arrived")
        .collect::<Vec<_>>();
    assert_eq!(arrivals.len(), 3);
    assert_eq!(
        arrivals
            .iter()
            .map(|event| event.data["node"].as_u64())
            .collect::<Vec<_>>(),
        vec![Some(7), Some(8), Some(9)]
    );
    assert!(arrivals.iter().all(|event| {
        event.data["authenticated"] == true && event.data["malformed_wire"] == false
    }));
    assert_eq!(run.report.authenticated_sybil_arrivals, 3);
    assert!(
        run.artifact
            .causal_ledger
            .iter()
            .any(|entry| { entry.stage == "authenticated-identities" && entry.count == 1 })
    );
    assert!(
        run.artifact
            .manifest
            .fidelity
            .approximations
            .iter()
            .any(|item| { item.method == "authenticated-sybil-admission-v1" })
    );
}

#[test]
fn root_grinding_sybils_charge_budgeted_operations_and_change_root() {
    let run = IndividualEngine
        .run_plan(&sybil_campaign(2, 2, "lower-than-current-root"))
        .unwrap();
    assert_eq!(run.report.authenticated_sybil_arrivals, 2);
    assert_eq!(run.report.identity_generation_trials, 8);
    let final_arrival = run
        .artifact
        .event_trace
        .iter()
        .rev()
        .find(|event| event.kind == "input.authenticated-sybil-arrived")
        .unwrap();
    assert_eq!(final_arrival.data["address"], run.report.final_root);
}

#[test]
fn sybil_identity_budget_is_rejected_before_execution() {
    assert!(matches!(
        IndividualEngine.run_plan(&sybil_campaign(4, 3, "uniform-valid")),
        Err(RunError::BudgetExhausted {
            required: 4,
            available: 3
        })
    ));
}
