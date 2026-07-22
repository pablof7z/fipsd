use super::*;
use crate::IndividualEngine;
use fips_model::normalize_str;

fn campaign(bloom: &str, traffic: &str) -> NormalizedPlan {
    normalize_str(&format!(
            r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {{name: m2-test}}
seed: 7
engine: {{modes: compact-discrete-event, deterministic: true}}
scale: {{nodes: 12}}
topology: {{generator: chain, average_degree: 2}}
identities:
  initial: {{distribution: uniform-128}}
  arrivals:
    count: 2
    schedule: {{start: 2s, interval: 499ms}}
    address_policy: strictly-lower-than-current-root
    attachment: current-root
    attacker_budget: {{mode: bounded, operations: 2, identities: 2}}
transports: {{assignment: all-udp}}
links:
  latency: 1ms
  bandwidth_bps: 10000000
  loss_ppm: 0
  duplication_ppm: 0
  ordering: stream
  mtu_bytes: 1500
  queue_bytes: 1048576
  drop_policy: tail-drop
resources:
  assignment: heterogeneous
  node_profiles:
    - {{name: baseline, cpu_units: 1000, memory_bytes: 1073741824, queue_bytes: 1048576, table_entries: 1000}}
events: [{{id: arrivals, action: introduce-lower-root-identities}}]
protocol:
  variant: fips-80c956a-baseline
  parameters:
    tree_announce_debounce: 500ms
    bloom_update_debounce: 500ms
    bloom_max_fpr_ppm: 200000
    coord_cache_entries: 16
    coord_cache_ttl: 5s
    lookup_ttl: 64
    lookup_attempts: 3
traffic:
  model: {traffic}
  rate_bps: 4096000
  payload_bytes: 512
  parameters: {{flow_count: 64}}
fidelity:
  protocol: semantic-exact
  serialization: executable-codec
  bloom: {bloom}
  crypto: operation-count
  billion_node_representation: not-requested
accounting: {{causal_lineage: true, transport_overhead: true, network_overhead: configured, reconcile_serialized_frames: true}}
instrumentation:
  root_agreement_by_depth: true
  transition_stages: true
  causal_cost_ledger: true
  queue_wait: true
  control_and_useful_bytes: true
  quiescence_markers: [root, tree, bloom, lookup, data-plane]
assertions: [{{always: {{condition: no_forwarding_loops}}}}]
objectives: {{maximize: [data_plane_stall_duration]}}
"#
        ))
        .unwrap()
}

#[test]
fn coupled_recovery_is_byte_stable_and_reconciles_all_layers() {
    let plan = campaign("exact-bits", "uniform-random");
    let root = IndividualEngine.run_plan(&plan).unwrap().report;
    let first = RecoveryEngine.run(&plan, &root).unwrap();
    let second = RecoveryEngine.run(&plan, &root).unwrap();
    assert_eq!(first, second);
    assert!(first.costs.frames_reconcile);
    assert!(first.costs.projections_reconcile);
    assert!(first.traffic.delivered_useful_bytes > 0);
    assert!(first.cache.invalidations > 0);
    assert!(first.resources.maximum_queue_wait_ns > 0);
    assert!(first.assertions.iter().all(|item| item.outcome == "pass"));
}

#[test]
fn every_bloom_fidelity_is_explicit_in_the_report() {
    for mode in ["exact-bits", "sparse-bits", "occupancy"] {
        let plan = campaign(mode, "idle");
        let root = IndividualEngine.run_plan(&plan).unwrap().report;
        let report = RecoveryEngine.run(&plan, &root).unwrap();
        assert!(report.fidelity_statement.contains(match mode {
            "exact-bits" => "exact packed",
            "sparse-bits" => "sparse exact",
            _ => "statistical Bloom occupancy",
        }));
    }
}
