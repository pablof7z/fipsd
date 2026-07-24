use super::*;

fn config(model: TrafficModel) -> TrafficConfig {
    TrafficConfig {
        model,
        nodes: 16,
        flow_count: 100,
        payload_bytes: 1_000,
        rate_bps: 8_000_000,
        interval_ns: 1_000_000,
        segments_per_stream: 4,
        burst_size: 8,
        burst_interval_ns: 50_000_000,
        seed: 77,
        transfers: Vec::new(),
    }
}

#[test]
fn every_traffic_model_is_seed_stable_and_has_no_self_flows() {
    let models = [
        TrafficModel::Idle,
        TrafficModel::UniformRandom,
        TrafficModel::Permutation,
        TrafficModel::AllToAll,
        TrafficModel::Zipf,
        TrafficModel::Incast,
        TrafficModel::Outcast,
        TrafficModel::ElephantsAndMice,
        TrafficModel::PersistentStreams,
        TrafficModel::ExplicitTransfers,
        TrafficModel::Bursty,
        TrafficModel::CrossCut,
        TrafficModel::SessionChurn,
        TrafficModel::PayloadSweep,
    ];
    for model in models {
        let first = TrafficPlan::generate(&config(model)).unwrap();
        let second = TrafficPlan::generate(&config(model)).unwrap();
        assert_eq!(first, second, "{model:?}");
        assert!(
            first
                .flows
                .iter()
                .all(|flow| flow.source != flow.destination),
            "{model:?}"
        );
    }
}

#[test]
fn explicit_transfer_chunks_cover_every_byte_between_authored_endpoints() {
    let mut input = config(TrafficModel::ExplicitTransfers);
    input.transfers = vec![TransferSpec {
        id: "download".to_owned(),
        source: 0,
        destination: 2,
        total_bytes: 2_500_001,
        visualization_chunk_bytes: 1_000_000,
        start_ns: 900,
    }];
    let plan = TrafficPlan::generate(&input).unwrap();
    assert_eq!(plan.flows.len(), 3);
    assert_eq!(plan.offered_useful_bytes, 2_500_001);
    assert!(
        plan.flows
            .iter()
            .all(|flow| { flow.source == 0 && flow.destination == 2 })
    );
    assert_eq!(
        plan.flows
            .iter()
            .map(|flow| flow.offered_at_ns)
            .collect::<Vec<_>>(),
        vec![900, 1_000_000_900, 2_000_000_900]
    );
    assert_eq!(plan.flows[2].useful_payload_bytes, 500_001);
    assert!(matches!(
        &plan.flows[2].shape,
        FlowShape::ApplicationTransfer {
            transfer_id,
            chunk_index: 2,
            chunk_count: 3,
            byte_end: 2_500_001,
            ..
        } if transfer_id == "download"
    ));
}

#[test]
fn persistent_streams_are_segmented_interleaved_and_session_bounded() {
    let mut input = config(TrafficModel::PersistentStreams);
    input.flow_count = 2;
    input.segments_per_stream = 3;
    let plan = TrafficPlan::generate(&input).unwrap();
    assert_eq!(plan.flows.len(), 6);
    assert_eq!(plan.session_setups, 2);
    assert_eq!(plan.session_teardowns, 2);
    assert_eq!(
        plan.flows
            .iter()
            .map(|flow| flow.offered_at_ns)
            .collect::<Vec<_>>(),
        vec![0, 1_000_000, 2_000_000, 3_000_000, 4_000_000, 5_000_000]
    );
    assert!(matches!(
        plan.flows[0].shape,
        FlowShape::StreamSegment {
            segment_index: 0,
            segment_count: 3,
            ..
        }
    ));
    assert_eq!(plan.flows[0].session_action, SessionAction::Setup);
    assert_eq!(plan.flows[4].session_action, SessionAction::Teardown);
}

#[test]
fn burst_members_share_offer_times_and_retain_partial_final_burst() {
    let mut input = config(TrafficModel::Bursty);
    input.flow_count = 10;
    input.burst_size = 4;
    let plan = TrafficPlan::generate(&input).unwrap();
    assert_eq!(
        plan.flows
            .iter()
            .map(|flow| flow.offered_at_ns)
            .collect::<Vec<_>>(),
        vec![
            0,
            0,
            0,
            0,
            50_000_000,
            50_000_000,
            50_000_000,
            50_000_000,
            100_000_000,
            100_000_000
        ]
    );
    assert!(matches!(
        plan.flows[9].shape,
        FlowShape::BurstMember {
            burst_index: 2,
            member_index: 1,
            member_count: 2
        }
    ));
}

#[test]
fn control_only_and_saturated_data_baselines_are_reproducible() {
    let idle = TrafficPlan::generate(&config(TrafficModel::Idle)).unwrap();
    assert_eq!(idle.offered_useful_bytes, 0);
    let data = TrafficPlan::generate(&config(TrafficModel::AllToAll)).unwrap();
    assert_eq!(data.flows.len(), 16 * 15);
    assert_eq!(
        data.offered_useful_bytes,
        data.flows
            .iter()
            .map(|flow| flow.useful_payload_bytes)
            .sum::<u64>()
    );
    assert_ne!(data.offered_useful_bytes, data.setup_message_bytes);
}

#[test]
fn session_churn_exposes_setup_and_teardown_separately() {
    let plan = TrafficPlan::generate(&config(TrafficModel::SessionChurn)).unwrap();
    assert_eq!(plan.session_setups, 50);
    assert_eq!(plan.session_teardowns, 50);
    assert_eq!(plan.setup_message_bytes, 50 * 176);
}
