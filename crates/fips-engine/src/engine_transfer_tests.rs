use super::*;
use crate::StreamEnqueueRequest;

#[test]
fn explicit_500_mb_download_routes_through_the_middle_and_reconciles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan =
        fips_model::normalize_path(&root.join("examples/three-node-file-transfer.yaml")).unwrap();
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let traffic = run.report.routed_traffic.unwrap();
    assert_eq!(traffic.offered_useful_bytes, 500_000_000);
    assert_eq!(traffic.delivered_useful_bytes, 500_000_000);
    assert_eq!(traffic.lost_useful_bytes, 0);
    let offers = run
        .artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "data.flow-offered")
        .collect::<Vec<_>>();
    assert_eq!(offers.len(), 500);
    assert!(
        offers
            .iter()
            .all(|event| event.data["path"] == json!([0, 1, 2]))
    );
    let due = run
        .artifact
        .event_trace
        .iter()
        .find(|event| event.kind == "data.frame-due")
        .unwrap();
    assert!(due.data["packet_count"].as_u64().unwrap() > 1);
    assert_eq!(due.data["shape"]["transfer_id"], "requested-download");
    assert!(
        run.artifact
            .manifest
            .fidelity
            .approximations
            .iter()
            .any(|item| item.method == "aggregated-reliable-stream-packetization-v1")
    );
}

#[test]
fn future_transfer_chunks_adopt_a_prompted_link_change_and_root_arrival() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut plan =
        fips_model::normalize_path(&root.join("examples/three-node-file-transfer.yaml")).unwrap();
    plan.campaign["events"] = json!([
        {
            "id": "spotty-b-c",
            "at": {"nanoseconds": 10_000_000_000_u64},
            "action": "set-link-conditions",
            "target": 1,
            "parameters": {
                "bandwidth_bps": 1_000_000,
                "latency": {"nanoseconds": 200_000_000_u64},
                "jitter": {"nanoseconds": 100_000_000_u64},
                "loss_ppm": 100_000
            }
        },
        {
            "id": "new-root",
            "at": {"nanoseconds": 10_000_000_000_u64},
            "action": "introduce-lower-root-node",
            "target": 0
        }
    ]);
    plan.campaign["identities"]["arrivals"]["attacker_budget"]["identities"] = json!(1);
    plan.campaign["identities"]["arrivals"]["attacker_budget"]["operations"] = json!(1);
    plan.campaign["scale"]["nodes"] = json!(4);
    plan.campaign["topology"]["explicit_edges"] = json!([[0, 1], [1, 2], [0, 3]]);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let traffic = run.report.routed_traffic.as_ref().unwrap();
    assert_eq!(traffic.rejected_flows, 0);
    assert_eq!(traffic.delivered_useful_bytes, 500_000_000);
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "input.descending-root-arrival" && event.virtual_time_ns == 10_000_000_000
    }));
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "input.link-conditions-changed"
            && event.data["edge"] == 1
            && event.data["after"]["bandwidth_bps"] == 1_000_000
    }));
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "data.flow-offered" && event.virtual_time_ns > 10_000_000_000
    }));
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "data.frame-due"
            && event.virtual_time_ns > 10_000_000_000
            && event.data["edge"] == 1
            && event.data["bandwidth_bps"] == 1_000_000
    }));
}

#[test]
fn future_chunks_reroute_through_a_new_bridge_after_the_old_bridge_leaves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut plan =
        fips_model::normalize_path(&root.join("examples/three-node-file-transfer.yaml")).unwrap();
    plan.campaign["events"] = json!([
        {
            "id": "new-bridge",
            "at": {"nanoseconds": 10_000_000_000_u64},
            "action": "introduce-node",
            "parameters": {"attachments": [0, 2]}
        },
        {
            "id": "old-bridge-leaves",
            "at": {"nanoseconds": 20_000_000_000_u64},
            "action": "disappear-node",
            "target": 1
        }
    ]);
    plan.campaign["identities"]["arrivals"]["attacker_budget"]["identities"] = json!(1);
    plan.campaign["identities"]["arrivals"]["attacker_budget"]["operations"] = json!(1);
    plan.campaign["scale"]["nodes"] = json!(4);
    plan.campaign["topology"]["explicit_edges"] = json!([[0, 1], [1, 2], [0, 3], [2, 3]]);
    let run = IndividualEngine.run_plan(&plan).unwrap();
    let traffic = run.report.routed_traffic.as_ref().unwrap();
    assert!(traffic.rejected_flows > 0);
    assert_eq!(
        traffic.delivered_useful_bytes + traffic.lost_useful_bytes,
        500_000_000
    );
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "input.node-arrived"
            && event.data["node"] == 3
            && event.data["targets"] == json!([0, 2])
    }));
    assert!(run.artifact.event_trace.iter().any(|event| {
        event.kind == "data.flow-offered"
            && event.virtual_time_ns > 20_000_000_000
            && event.data["path"] == json!([0, 3, 2])
    }));
}

#[test]
fn aggregate_stream_packetization_reconciles_transmit_loss_and_delivery() {
    let mut links = LinkService::uniform(
        7,
        1,
        LinkConfig {
            bandwidth_bps: 10_000_000,
            loss_ppm: 10_000,
            mtu_bytes: 1_500,
            ordering: LinkOrdering::Stream,
            ..LinkConfig::default()
        },
    );
    let result = links
        .enqueue_stream(StreamEnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            useful_payload_bytes: 1_000_000,
            protocol_overhead_bytes: 106,
            now_ns: 0,
        })
        .unwrap();
    assert!(result.packet_count > 1);
    assert!(result.retransmitted_packets > 0);
    links.record_delivery(&result.delivery, 1_000_000).unwrap();
    links.reconcile().unwrap();
}
