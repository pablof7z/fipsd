use super::GraphRecoveryRuntime;
use crate::*;
use fips_model::{NormalizedPlan, normalize_str};
use serde_json::json;
use sha2::{Digest, Sha256};

fn recovery_campaign(model: &str, nodes: u32, flows: u64, mtu: u64) -> NormalizedPlan {
    let source = r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: graph-recovery-test}
seed: 91
engine: {modes: compact-discrete-event, deterministic: true}
scale: {nodes: NODES}
topology: {generator: chain, average_degree: 2}
identities:
  initial: {distribution: uniform-128}
  arrivals:
    count: 0
    schedule: {start: 20s, interval: 1s}
    address_policy: strictly-lower-than-current-root
    attachment: random
transports:
  assignment: random-mixed
  profiles:
    - {name: test-media, type: wifi, mtu_bytes: MTU, latency: 1ms, bandwidth_bps: 100000000, weight: 1}
links: {latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, duplication_ppm: 0, ordering: datagram, mtu_bytes: 1500, queue_bytes: 1048576}
resources: {assignment: uniform}
events: []
protocol:
  variant: fips-80c956a-baseline
  parameters: {tree_announce_debounce: 500ms, lookup_ttl: 64, lookup_attempts: 3, lookup_backoff: 10ms, lookup_jitter: 0ms, coord_cache_entries: 64, coord_cache_ttl: 5s}
traffic:
  model: MODEL
  rate_bps: 10000
  payload_bytes: 512
  parameters: {flow_count: FLOWS, start_ns: 3000000000}
fidelity: {protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}
accounting: {causal_lineage: true, transport_overhead: true, network_overhead: configured, reconcile_serialized_frames: true}
instrumentation: {transition_stages: true, control_and_useful_bytes: true, quiescence_markers: [root, tree, lookup]}
assertions: []
objectives: {maximize: [useful-throughput]}
"#;
    normalize_str(
        &source
            .replace("NODES", &nodes.to_string())
            .replace("FLOWS", &flows.to_string())
            .replace("MTU", &mtu.to_string())
            .replace("MODEL", model),
    )
    .unwrap()
}

fn with_events(mut plan: NormalizedPlan, events: serde_json::Value) -> NormalizedPlan {
    plan.campaign["events"] = events;
    plan.campaign_sha256 = hex::encode(Sha256::digest(serde_json::to_vec(&plan.campaign).unwrap()));
    plan
}

#[test]
fn mixed_lookup_session_and_payload_use_every_path_hop() {
    let run = IndividualEngine
        .run_plan(&recovery_campaign("cross-min-cut", 6, 1, 1_500))
        .unwrap();
    assert!(run.recovery_report.is_none());
    let due = |kind: &str, message: &str| {
        run.artifact
            .event_trace
            .iter()
            .filter(|event| event.kind == kind && event.data["message"] == json!(message))
            .count()
    };
    assert_eq!(due("lookup.frame-due", "lookup-request"), 3);
    assert_eq!(due("lookup.frame-due", "lookup-response"), 3);
    assert_eq!(due("session.frame-due", "session-setup"), 3);
    assert_eq!(due("session.frame-due", "session-ack"), 3);
    assert_eq!(due("data.frame-due", "session-data"), 3);
    let recovery = run.report.graph_recovery.unwrap();
    assert_eq!(
        (recovery.lookups, recovery.successes, recovery.failures),
        (1, 1, 0)
    );
    assert_eq!((recovery.session_setups, recovery.session_acks), (1, 1));
    assert!(recovery.reconciles());
}

#[test]
fn coordinate_cache_and_session_are_reused_then_torn_down() {
    let run = IndividualEngine
        .run_plan(&recovery_campaign("session-churn", 4, 2, 1_500))
        .unwrap();
    let offers = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "data.flow-offered")
        .collect::<Vec<_>>();
    assert_eq!(offers[0].data["cache"], json!("cache-miss"));
    assert_eq!(offers[1].data["cache"], json!("cache-hit"));
    let recovery = run.report.graph_recovery.unwrap();
    assert_eq!(recovery.lookups, 1);
    assert_eq!(recovery.cache.hits, 1);
    assert_eq!(recovery.session_setups, 1);
    assert_eq!(recovery.teardowns, 1);
    assert_eq!(run.report.routed_traffic.unwrap().delivered_flows, 2);
}

#[test]
fn response_mtu_failure_retries_and_reconciles_without_payload() {
    let run = IndividualEngine
        .run_plan(&recovery_campaign("cross-min-cut", 4, 1, 120))
        .unwrap();
    let rejected = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "lookup.frame-due")
        .filter(|event| {
            event.data["outcome"] == json!("retry") || event.data["outcome"] == json!("failed")
        })
        .count();
    assert_eq!(rejected, 3);
    let recovery = run.report.graph_recovery.unwrap();
    assert_eq!((recovery.attempts, recovery.retries), (3, 2));
    assert_eq!((recovery.successes, recovery.failures), (0, 1));
    assert_eq!(recovery.session_setups, 0);
    assert!(recovery.reconciles());
    assert_eq!(run.report.routed_traffic.unwrap().rejected_flows, 1);
}

#[test]
fn topology_change_invalidates_coordinates_and_disrupts_sessions() {
    let plan = recovery_campaign("session-churn", 4, 2, 1_500);
    let mut runtime = GraphRecoveryRuntime::from_plan(&plan, 4).unwrap().unwrap();
    runtime.insert_cache(0, [2; 16], [9; 16], vec![0, 1, 2], 0);
    runtime.insert_session(0, 2, vec![0, 1, 2]);
    assert_eq!(runtime.invalidate_root([9; 16]), 1);
    assert_eq!(runtime.disrupt_sessions_for_node(2), 1);
    let counters = runtime.snapshot_counters();
    assert_eq!(counters.cache.invalidations, 1);
    assert_eq!(counters.session_disruptions, 1);
}

#[test]
fn synchronized_rekey_wave_operation_counts_every_live_session() {
    let plan = with_events(
        recovery_campaign("uniform-random", 6, 4, 1_500),
        json!([{
            "id": "wave",
            "action": "synchronized-session-rekey",
            "at": {"nanoseconds": 5_000_000_000_u64}
        }]),
    );
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let request = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.session-rekey-wave")
        .unwrap();
    let completions = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "session.rekey-completed")
        .collect::<Vec<_>>();
    assert!(!completions.is_empty());
    assert_eq!(
        request.data["scheduled_rekeys"].as_u64().unwrap() as usize,
        completions.len()
    );
    assert!(completions.iter().all(|event| {
        event.causal_parent.as_deref() == Some(request.event_id.as_str())
            && event.data["crypto_fidelity"] == "operation-counted-no-wire-frame"
    }));
    assert_eq!(
        run.report.graph_recovery.unwrap().rekeys,
        completions.len() as u64
    );
}

#[test]
fn cache_expiry_precedes_a_same_timestamp_lookup_wave() {
    let plan = with_events(
        recovery_campaign("uniform-random", 6, 6, 1_500),
        json!([
            {
                "id": "expire",
                "action": "expire-coordinate-cache",
                "at": {"nanoseconds": 6_000_000_000_u64}
            },
            {
                "id": "herd",
                "action": "simultaneous-lookups",
                "at": {"nanoseconds": 6_000_000_000_u64},
                "parameters": {"count": 4}
            }
        ]),
    );
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let expiry = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.coordinate-cache-expired")
        .unwrap();
    let wave = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.lookup-wave")
        .unwrap();
    assert!(expiry.ordinal < wave.ordinal);
    assert!(expiry.data["invalidated_entries"].as_u64().unwrap() > 0);
    assert_eq!(wave.data["scheduled_lookups"], 4);
    let offers = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| {
            event.kind == "data.flow-offered"
                && event.data["flow_id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("lookup-wave-herd-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(offers.len(), 4);
    assert!(offers.iter().all(|event| {
        event.virtual_time_ns == wave.virtual_time_ns
            && event.causal_parent.as_deref() == Some(wave.event_id.as_str())
            && event.data["cache"] == "cache-miss"
    }));
    let traffic = run.report.routed_traffic.unwrap();
    assert_eq!(
        traffic.offered_flows,
        traffic.delivered_flows + traffic.rejected_flows
    );
}

#[test]
fn transport_class_failure_and_restore_disrupt_then_reconnect_edges() {
    let plan = with_events(
        recovery_campaign("uniform-random", 6, 6, 1_500),
        json!([
            {
                "id": "media-down",
                "action": "fail-transport-class",
                "target": "test-media",
                "at": {"nanoseconds": 6_000_000_000_u64}
            },
            {
                "id": "media-up",
                "action": "restore-transport-class",
                "target": "test-media",
                "at": {"nanoseconds": 7_000_000_000_u64}
            }
        ]),
    );
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let failed = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.transport-class-failed")
        .unwrap();
    let restored = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "input.transport-class-restored")
        .unwrap();
    let failed_edges = failed.data["changed_edges"].as_array().unwrap();
    let restored_edges = restored.data["changed_edges"].as_array().unwrap();
    assert_eq!(failed.data["profile"], "test-media");
    assert_eq!(failed.data["affected_nodes"].as_array().unwrap().len(), 6);
    assert!(!failed_edges.is_empty());
    assert_eq!(failed_edges.len(), restored_edges.len());
    assert!(failed.ordinal < restored.ordinal);
    assert!(
        run.report
            .assertions
            .iter()
            .all(|assertion| assertion.outcome == "pass")
    );
}

#[test]
fn unknown_event_actions_fail_instead_of_silently_disappearing() {
    let plan = with_events(
        recovery_campaign("idle", 4, 0, 1_500),
        json!([{
            "action": "typo-that-must-not-disappear",
            "at": {"nanoseconds": 1_000_000}
        }]),
    );
    let error = IndividualEngine.run_plan(&plan).unwrap_err().to_string();
    assert!(error.contains("unsupported individual-engine event action"));
    assert!(error.contains("/events/0"));
}
