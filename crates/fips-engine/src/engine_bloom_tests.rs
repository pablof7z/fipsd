use super::*;
use fips_model::normalize_str;

fn bloom_campaign() -> NormalizedPlan {
    bloom_campaign_variant(
        "exact-bits",
        "{name: wifi, type: wifi, mtu_bytes: 1500, latency: 8ms, bandwidth_bps: 100000000, weight: 1}",
    )
}

fn bloom_campaign_with_fidelity(fidelity: &str) -> NormalizedPlan {
    bloom_campaign_variant(
        fidelity,
        "{name: wifi, type: wifi, mtu_bytes: 1500, latency: 8ms, bandwidth_bps: 100000000, weight: 1}",
    )
}

fn bloom_campaign_variant(fidelity: &str, profile: &str) -> NormalizedPlan {
    let source = r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: streamed-bloom-test}
seed: 37
engine: {modes: compact-discrete-event, deterministic: true}
scale: {nodes: 6}
topology: {generator: chain, average_degree: 2}
identities:
  initial: {distribution: uniform-128}
  arrivals:
    count: 1
    schedule: {start: 2s, interval: 1s}
    address_policy: strictly-lower-than-current-root
    attachment: leaf
transports:
  assignment: random-mixed
  profiles:
    - PROFILE
links: {latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, duplication_ppm: 0, ordering: datagram, mtu_bytes: 1500, queue_bytes: 1048576}
resources: {assignment: uniform}
events: []
protocol:
  variant: fips-80c956a-baseline
  parameters: {tree_announce_debounce: 500ms, bloom_update_debounce: 500ms}
traffic: {model: idle}
fidelity: {protocol: semantic-exact, serialization: executable-codec, bloom: BLOOM_MODE, crypto: operation-count, billion_node_representation: not-requested}
accounting: {causal_lineage: true, transport_overhead: true, network_overhead: configured, reconcile_serialized_frames: true}
instrumentation: {transition_stages: true, control_and_useful_bytes: true, quiescence_markers: [root, tree, bloom]}
assertions: []
objectives: {maximize: [control_bytes_per_root_arrival]}
"#;
    normalize_str(
        &source
            .replace("BLOOM_MODE", fidelity)
            .replace("PROFILE", profile),
    )
    .unwrap()
}

#[test]
fn occupancy_bloom_stream_remains_explicitly_approximate() {
    let run = IndividualEngine
        .run_plan(&bloom_campaign_with_fidelity("occupancy"))
        .unwrap();
    assert_eq!(
        run.artifact.manifest.fidelity.bloom,
        BloomFidelity::Occupancy
    );
    assert!(
        run.artifact
            .manifest
            .fidelity
            .approximations
            .iter()
            .any(|item| item.method == "seeded-bloom-occupancy-v1")
    );
}

#[test]
fn bluetooth_mtu_rejects_full_filter_on_the_actual_edge() {
    let plan = bloom_campaign_variant(
        "exact-bits",
        "{name: bluetooth, type: ble, mtu_bytes: 244, latency: 20ms, bandwidth_bps: 1000000, weight: 1}",
    );
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let rejected = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "bloom.filter-due" && event.data["rejected"].as_str().is_some())
        .unwrap();
    assert_eq!(rejected.data["mtu_bytes"], json!(244));
    assert_eq!(rejected.data["from_transport"], json!("bluetooth"));
    assert_eq!(rejected.data["to_transport"], json!("bluetooth"));
    let counters = run.report.bloom_propagation.unwrap();
    assert_eq!(counters.wave.sent, 0);
    assert!(counters.wave.rejected > 0);
    assert!(counters.reconciles());
}

#[test]
fn bloom_only_recovery_streams_over_real_mixed_profile_edges() {
    let run = IndividualEngine.run_plan(&bloom_campaign()).unwrap();
    assert!(run.recovery_report.is_none());
    let due = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "bloom.filter-due")
        .collect::<Vec<_>>();
    assert!(!due.is_empty());
    assert!(due.iter().all(|event| {
        event.data["frame_bytes"] == json!(FILTER_ANNOUNCE_FMP_BYTES)
            && event.data["from_transport"] == json!("wifi")
            && event.data["to_transport"] == json!("wifi")
    }));
    let maximum_cardinality = due
        .iter()
        .filter_map(|event| event.data["estimated_cardinality"].as_u64())
        .max()
        .unwrap();
    assert!(maximum_cardinality >= 4);
    let counters = run.report.bloom_propagation.unwrap();
    assert!(counters.delivered_frames > 0);
    assert!(counters.reconciles());
    assert!(run.report.assertions.iter().any(|assertion| {
        assertion.id == "bloom-propagation-reconciliation" && assertion.outcome == "pass"
    }));
}
