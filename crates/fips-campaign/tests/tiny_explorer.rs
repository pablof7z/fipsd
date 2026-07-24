use fips_campaign::{TinyExplorerConfig, TinyStateExplorer};
use fips_model::normalize_str;

const CAMPAIGN: &str = r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: tiny-lifecycle-orders}
seed: 77
engine: {modes: compact-discrete-event, deterministic: true}
scale: {nodes: 4}
topology: {generator: chain}
identities: {initial: {distribution: uniform-128}}
transports: {assignment: all-udp}
links:
  {latency: 1ms, bandwidth_bps: 100000000, loss_ppm: 0, ordering: stream,
   mtu_bytes: 9000, queue_bytes: 1048576}
events:
  - {id: disappear-one, action: disappear-node, target: 1, at: 1s}
  - {id: reappear-one, action: reappear-node, target: 1, at: 2s}
protocol: {variant: fips-80c956a-baseline, parameters: {tree_announce_debounce: 1ms}}
traffic: {model: idle}
fidelity:
  {protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits,
   crypto: operation-count, billion_node_representation: not-requested}
instrumentation: {transition_stages: true}
assertions:
  - {eventually: {condition: all_connected_nodes_agree_on_minimum_root,
      after_input_quiescence: 1s}}
  - {always: {condition: no_forwarding_loops}}
objectives: {maximize: [control-bytes]}
"#;

#[test]
fn every_lifecycle_order_is_explored_and_counterexamples_are_replayable() {
    let plan = normalize_str(CAMPAIGN).unwrap();
    let config = TinyExplorerConfig {
        maximum_nodes: 4,
        maximum_actions: 2,
        step_ns: 1_000_000,
    };
    let left = TinyStateExplorer.explore(&plan, config.clone()).unwrap();
    let right = TinyStateExplorer.explore(&plan, config).unwrap();

    assert_eq!(left, right);
    assert!(left.exhaustive);
    assert_eq!(left.expected_permutations, 2);
    assert_eq!(left.explored_permutations, 2);
    assert_eq!(left.outcomes.len(), 2);
    assert!(!left.counterexamples.is_empty());

    for counterexample in &left.counterexamples {
        match fips_engine::IndividualEngine.run_plan(&counterexample.normalized_plan) {
            Ok(run) => assert!(
                run.artifact
                    .assertion_results
                    .iter()
                    .any(|assertion| assertion.outcome != "pass")
            ),
            Err(error) => assert_eq!(error.to_string(), counterexample.failure),
        }
    }
}
