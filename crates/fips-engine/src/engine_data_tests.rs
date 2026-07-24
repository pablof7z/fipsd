use super::*;
use fips_model::normalize_str;
use sha2::{Digest, Sha256};

fn routed_campaign() -> NormalizedPlan {
    normalize_str(
        r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: routed-data-test}
seed: 91
engine: {modes: compact-discrete-event, deterministic: true}
scale: {nodes: 6}
topology: {generator: chain, average_degree: 2}
identities:
  initial: {distribution: uniform-128}
  arrivals:
    count: 0
    schedule: {start: 2s, interval: 1s}
    address_policy: strictly-lower-than-current-root
    attachment: random
transports:
  assignment: random-mixed
  profiles:
    - {name: wifi, type: wifi, mtu_bytes: 1500, latency: 8ms, bandwidth_bps: 100000000, weight: 1}
links: {latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, duplication_ppm: 0, ordering: datagram, mtu_bytes: 1500, queue_bytes: 1048576}
resources: {assignment: uniform}
events: []
protocol: {variant: fips-80c956a-baseline, parameters: {tree_announce_debounce: 500ms}}
traffic:
  model: cross-min-cut
  rate_bps: 1000000
  payload_bytes: 512
  parameters: {flow_count: 1, start_ns: 500000000}
fidelity: {protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}
accounting: {causal_lineage: true, transport_overhead: true, network_overhead: configured, reconcile_serialized_frames: true}
instrumentation: {transition_stages: true, control_and_useful_bytes: true, quiescence_markers: [root, tree]}
assertions: []
objectives: {maximize: [useful-throughput]}
"#,
    )
    .unwrap()
}

fn routed_campaign_with_traffic(traffic: Value) -> NormalizedPlan {
    let mut plan = routed_campaign();
    plan.campaign["traffic"] = traffic;
    plan.campaign_sha256 = hex::encode(Sha256::digest(serde_json::to_vec(&plan.campaign).unwrap()));
    plan
}

#[test]
fn routed_payload_uses_every_stable_shortest_path_hop() {
    let run = IndividualEngine.run_plan(&routed_campaign()).unwrap();
    let offered = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "data.flow-offered")
        .unwrap();
    assert_eq!(offered.data["path"], json!([0, 1, 2, 3]));
    let due = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "data.frame-due")
        .collect::<Vec<_>>();
    assert_eq!(due.len(), 3);
    assert_eq!(due[0].data["from"], 0);
    assert_eq!(due[2].data["to"], 3);
    let counters = run.report.routed_traffic.unwrap();
    assert_eq!(counters.delivered_flows, 1);
    assert_eq!(counters.delivered_useful_bytes, 512);
    assert_eq!(counters.maximum_hops, 3);
    let last_data_ns = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind.starts_with("data."))
        .map(|event| event.virtual_time_ns)
        .max()
        .unwrap();
    let last_tree_ns = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| !event.kind.starts_with("data."))
        .map(|event| event.virtual_time_ns)
        .max()
        .unwrap();
    assert_eq!(counters.quiescence_ns, last_data_ns);
    assert_eq!(run.report.quiescence_ns, last_tree_ns);
    assert!(run.report.assertions.iter().any(|assertion| {
        assertion.id == "routed-traffic-reconciliation" && assertion.outcome == "pass"
    }));
    assert_eq!(run.artifact.manifest.fidelity.wire, WireFidelity::Modeled);
    assert!(
        run.artifact
            .manifest
            .fidelity
            .approximations
            .iter()
            .any(|item| { item.method == "routed-synthetic-session-data-v1" })
    );
}

#[test]
fn persistent_stream_segments_are_individually_routed_and_labeled() {
    let plan = routed_campaign_with_traffic(json!({
        "model": "persistent-streams",
        "rate_bps": 1_000_000,
        "payload_bytes": 512,
        "parameters": {
            "flow_count": 2,
            "segments_per_stream": 3,
            "start_ns": 500_000_000
        }
    }));
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let offers = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "data.flow-offered")
        .collect::<Vec<_>>();
    assert_eq!(offers.len(), 6);
    assert_eq!(offers[0].data["shape"]["kind"], "stream-segment");
    assert_eq!(offers[0].data["shape"]["stream_id"], "stream-00000000");
    assert_eq!(offers[0].data["shape"]["segment_index"], 0);
    assert_eq!(offers[4].data["shape"]["segment_index"], 2);
    let counters = run.report.routed_traffic.unwrap();
    assert_eq!(counters.offered_flows, 6);
    assert_eq!(counters.delivered_flows, 6);
    assert_eq!(counters.delivered_useful_bytes, 6 * 512);
}

#[test]
fn burst_process_schedules_simultaneous_offers_at_explicit_intervals() {
    let plan = routed_campaign_with_traffic(json!({
        "model": "bursty",
        "rate_bps": 1_000_000,
        "payload_bytes": 256,
        "parameters": {
            "flow_count": 5,
            "burst_size": 3,
            "burst_interval_ns": 100_000_000,
            "start_ns": 500_000_000
        }
    }));
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let offers = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "data.flow-offered")
        .collect::<Vec<_>>();
    assert_eq!(
        offers
            .iter()
            .map(|event| event.virtual_time_ns)
            .collect::<Vec<_>>(),
        vec![
            500_000_000,
            500_000_000,
            500_000_000,
            600_000_000,
            600_000_000
        ]
    );
    assert_eq!(offers[0].data["shape"]["kind"], "burst-member");
    assert_eq!(offers[4].data["shape"]["member_count"], 2);
}
