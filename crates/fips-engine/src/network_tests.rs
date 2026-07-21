use super::*;

#[test]
fn single_link_matches_analytical_delivery_time_and_bytes() {
    let config = LinkConfig {
        latency_ns: 2_000_000,
        bandwidth_bps: 1_000_000,
        transport_overhead_bytes: 0,
        ordering: LinkOrdering::Stream,
        ..LinkConfig::default()
    };
    let mut service = LinkService::uniform(7, 1, config);
    let result = service
        .enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class: LinkClass::Control,
            frame_bytes: 1_000,
            useful_payload_bytes: 0,
            now_ns: 10,
        })
        .unwrap();
    assert_eq!(result.transmitted_bytes, 1_000);
    assert_eq!(result.deliveries[0].deliver_at_ns, 10_000_010);
    service.record_delivery(&result.deliveries[0], 0).unwrap();
    service.reconcile().unwrap();
}

#[test]
fn mtu_and_queue_failures_are_typed_and_counted() {
    let config = LinkConfig {
        mtu_bytes: 100,
        queue_bytes: 150,
        transport_overhead_bytes: 0,
        bandwidth_bps: 1,
        ..LinkConfig::default()
    };
    let mut service = LinkService::uniform(1, 1, config);
    assert!(matches!(
        service.enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class: LinkClass::Control,
            frame_bytes: 101,
            useful_payload_bytes: 0,
            now_ns: 0,
        }),
        Err(LinkError::MtuExceeded { .. })
    ));
    service
        .enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class: LinkClass::Control,
            frame_bytes: 100,
            useful_payload_bytes: 0,
            now_ns: 0,
        })
        .unwrap();
    assert!(matches!(
        service.enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class: LinkClass::Control,
            frame_bytes: 100,
            useful_payload_bytes: 0,
            now_ns: 0,
        }),
        Err(LinkError::QueueFull { .. })
    ));
    let counters = service.counters(0, 0, 1);
    assert_eq!(counters.accepted_frames, 1);
    assert_eq!(counters.rejected_frames, 2);
    assert_eq!(counters.rejected_bytes, 201);
}

#[test]
fn seeded_loss_and_duplication_are_replayable() {
    let config = LinkConfig {
        loss_ppm: 300_000,
        duplication_ppm: 500_000,
        queue_bytes: 1_000_000,
        ..LinkConfig::default()
    };
    let mut left = LinkService::uniform(99, 1, config.clone());
    let mut right = LinkService::uniform(99, 1, config);
    for sequence in 0..20 {
        assert_eq!(
            left.enqueue(EnqueueRequest {
                edge_id: 0,
                from: 0,
                to: 1,
                class: LinkClass::Control,
                frame_bytes: 100,
                useful_payload_bytes: 0,
                now_ns: sequence,
            }),
            right.enqueue(EnqueueRequest {
                edge_id: 0,
                from: 0,
                to: 1,
                class: LinkClass::Control,
                frame_bytes: 100,
                useful_payload_bytes: 0,
                now_ns: sequence,
            })
        );
    }
}

#[test]
fn control_and_useful_payload_share_capacity_but_not_accounting() {
    let config = LinkConfig {
        ordering: LinkOrdering::Stream,
        bandwidth_bps: 8_000,
        latency_ns: 0,
        transport_overhead_bytes: 0,
        ..LinkConfig::default()
    };
    let mut service = LinkService::uniform(1, 1, config);
    let control = service
        .enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class: LinkClass::Control,
            frame_bytes: 1_000,
            useful_payload_bytes: 0,
            now_ns: 0,
        })
        .unwrap();
    let useful = service
        .enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class: LinkClass::UsefulPayload,
            frame_bytes: 1_100,
            useful_payload_bytes: 1_000,
            now_ns: 0,
        })
        .unwrap();
    assert_eq!(control.deliveries[0].deliver_at_ns, 1_000_000_000);
    assert_eq!(useful.deliveries[0].deliver_at_ns, 2_100_000_000);
    service.record_delivery(&control.deliveries[0], 0).unwrap();
    service
        .record_delivery(&useful.deliveries[0], 1_000)
        .unwrap();
    let counters = service.counters(0, 0, 1);
    assert_eq!(counters.delivered_bytes, 2_100);
    assert_eq!(counters.useful_payload_bytes, 1_000);
    service.reconcile().unwrap();
}
