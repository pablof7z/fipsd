use fips_campaign::{
    AdversaryAction, AdversaryPolicy, AttackerBudget, AttackerBudgetMode, ConnectivityClass,
    GeneratorConstraints, MediaKind, MediaOrdering, ProtocolDisposition, TransportAssignmentPolicy,
    assign_transports, builtin_profiles, execute_adversary, failover, generate_input,
    shrink_generated,
};

fn constraints(connectivity: ConnectivityClass) -> GeneratorConstraints {
    GeneratorConstraints {
        nodes: 64,
        connectivity,
        maximum_degree: 4,
        event_count: 14,
        minimum_interval_ns: 1_000_000,
    }
}

#[test]
fn property_generators_honor_constraints_and_compose_with_shrinking() {
    let connected = generate_input(7, constraints(ConnectivityClass::Connected)).unwrap();
    let disconnected = generate_input(7, constraints(ConnectivityClass::Disconnected)).unwrap();
    assert_eq!(connected.events.len(), 14);
    assert_ne!(connected.topology.edges, disconnected.topology.edges);
    assert!(connected.topology.edges.iter().all(|(a, b)| a != b));
    assert!(
        connected
            .events
            .windows(2)
            .all(|pair| pair[1].at_ns > pair[0].at_ns)
    );

    let million = generate_input(
        9,
        GeneratorConstraints {
            nodes: 1_000_000,
            ..constraints(ConnectivityClass::Connected)
        },
    )
    .unwrap();
    assert!(million.topology.symbolic_population);
    let shrunk = shrink_generated(million, |candidate| {
        candidate.topology.represented_nodes >= 16 && candidate.events.len() >= 2
    });
    assert!(shrunk.topology.represented_nodes < 1_000_000);
    assert!(shrunk.events.len() < 14);
}

#[test]
fn media_profiles_label_provenance_and_failover_lineage() {
    let profiles = builtin_profiles();
    assert_eq!(profiles.len(), 6);
    let udp = profiles
        .iter()
        .find(|profile| profile.kind == MediaKind::Udp)
        .unwrap();
    let tcp = profiles
        .iter()
        .find(|profile| profile.kind == MediaKind::Tcp)
        .unwrap();
    assert_eq!(udp.ordering, MediaOrdering::Datagram);
    assert_eq!(tcp.ordering, MediaOrdering::Stream);
    assert!(!udp.reliable && tcp.reliable);
    assert!(profiles.iter().all(|profile| {
        profile.effective_mtu_bytes > 0
            && profile.transport_overhead_bytes > 0
            && !profile.version.is_empty()
    }));

    let topology = generate_input(3, constraints(ConnectivityClass::Connected))
        .unwrap()
        .topology;
    let mut assignment = assign_transports(&topology, TransportAssignmentPolicy::Failover, 3);
    let edge = assignment.edge_profiles.keys().next().unwrap().clone();
    let target = assignment
        .profiles
        .values()
        .find(|profile| profile.kind == MediaKind::Tor)
        .unwrap()
        .id
        .clone();
    let outcome = failover(&mut assignment, &edge, &target, "flow:0001", 100).unwrap();
    assert_eq!(outcome.parent_causal_id, "flow:0001");
    assert_eq!(outcome.to_profile, target);
    assert!(outcome.reconnect_complete_ns > 100);
}

#[test]
fn authenticated_adversaries_expose_budget_and_interpretation() {
    let actions = [
        AdversaryAction {
            policy: AdversaryPolicy::IdentityGrinding,
            operations: 4,
            identities: 1,
            at_ns: 0,
        },
        AdversaryAction {
            policy: AdversaryPolicy::DishonestAncestry,
            operations: 4,
            identities: 1,
            at_ns: 1,
        },
    ];
    let report = execute_adversary(
        AttackerBudget {
            mode: AttackerBudgetMode::Bounded,
            maximum_operations: Some(6),
            maximum_identities: Some(2),
            calibrated_units_per_operation: None,
        },
        &actions,
    );
    assert!(report.authenticated && !report.malformed_wire_fuzzing);
    assert_eq!(report.operations_consumed, 4);
    assert_eq!(report.exhausted_actions, 1);
    assert_eq!(
        report.records[0].disposition,
        ProtocolDisposition::AcceptedProtocolBehavior
    );
    assert!(
        report.records[1]
            .rejection
            .as_ref()
            .unwrap()
            .contains("budget exhausted")
    );
    assert_eq!(
        report.ledger[1].causal_parent.as_deref(),
        Some("adversary:00000000")
    );
}
