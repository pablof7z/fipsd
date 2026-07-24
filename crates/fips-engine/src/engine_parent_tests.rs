use super::*;
use fips_model::normalize_str;

fn parent_campaign(event: &str, parameters: &str) -> NormalizedPlan {
    normalize_str(&format!(
        r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {{name: parent-intervention-test}}
seed: 77
engine: {{modes: compact-discrete-event, deterministic: true}}
scale: {{nodes: 5}}
topology:
  generator: explicit
  explicit_edges: [[0, 1], [0, 2], [1, 3], [2, 3], [3, 4]]
  average_degree: 2
identities: {{initial: {{distribution: uniform-128}}}}
transports: {{assignment: all-udp}}
links: {{latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, ordering: datagram, mtu_bytes: 9000, queue_bytes: 1048576}}
resources: {{assignment: uniform}}
events:
  - id: parent-test
    action: {event}
    at: 2s
    parameters: {parameters}
protocol:
  variant: fips-80c956a-baseline
  parameters:
    tree_announce_debounce: 500ms
    bloom_update_debounce: 500ms
traffic: {{model: idle}}
fidelity: {{protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}}
accounting: {{causal_lineage: true, reconcile_serialized_frames: true}}
instrumentation:
  transition_stages: true
  quiescence_markers: [root, tree, bloom]
assertions: []
objectives: {{maximize: [parent-switches]}}
"#
    ))
    .unwrap()
}

#[test]
fn ancestry_swap_uses_real_parent_transition_and_bloom_pipeline() {
    let plan = parent_campaign("swap-parent-ancestry", "{}");
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let swap = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.parent-ancestry-swapped")
        .unwrap();
    assert_eq!(swap.data["switched"], true);
    assert_ne!(swap.data["old_parent"], swap.data["new_parent"]);
    assert_ne!(swap.data["old_ancestry"], swap.data["new_ancestry"]);
    assert!(
        run.artifact.causal_ledger.iter().any(|entry| {
            entry.causal_id == "input:parent-test" && entry.stage == "state-mutated"
        })
    );
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "bloom.filter-due"
            && event
                .causal_parent
                .as_deref()
                .is_some_and(|parent| parent == swap.event_id)
    }));
}

#[test]
fn alternating_quality_flaps_without_hold_down() {
    let plan = parent_campaign(
        "alternate-parent-quality",
        "{cycles: 4, interval: 250ms, preferred_cost_ppm: 1000000, degraded_cost_ppm: 6000000}",
    );
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let pulses = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "input.parent-quality-alternated")
        .collect::<Vec<_>>();
    assert_eq!(pulses.len(), 4);
    assert!(pulses.iter().all(|event| event.data["switched"] == true));
    assert_eq!(pulses[0].data["cost_source"], "modeled-mmp-fixed-point");
}

#[test]
fn hold_down_suppresses_only_later_discretionary_flaps() {
    let mut plan = parent_campaign(
        "alternate-parent-quality",
        "{cycles: 4, interval: 250ms, preferred_cost_ppm: 1000000, degraded_cost_ppm: 6000000}",
    );
    plan.campaign["protocol"]["parameters"]["parent_hold_down"] =
        json!({"nanoseconds": 1_000_000_000_u64});
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let pulses = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "input.parent-quality-alternated")
        .collect::<Vec<_>>();
    assert_eq!(
        pulses
            .iter()
            .filter(|event| event.data["switched"] == true)
            .count(),
        1
    );
    assert_eq!(
        pulses
            .iter()
            .filter(|event| event.data["suppressed"] == true)
            .count(),
        3
    );
}
