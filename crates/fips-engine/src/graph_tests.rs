use super::*;

#[test]
fn generators_are_connected_and_seed_stable() {
    let cases = [
        (TopologyKind::Chain, 2),
        (TopologyKind::BalancedTree, 2),
        (TopologyKind::RandomRegular, 4),
        (TopologyKind::ScaleFree, 4),
    ];
    for (kind, degree) in cases {
        let left = GraphStore::generate(kind, 20, degree, 77, &[]).unwrap();
        let right = GraphStore::generate(kind, 20, degree, 77, &[]).unwrap();
        assert!(left.is_connected_active());
        assert_eq!(left.graph_sha256(), right.graph_sha256());
        assert_eq!(left, right);
    }
}

#[test]
fn random_regular_degree_constraint_is_exact() {
    let graph = GraphStore::generate(TopologyKind::RandomRegular, 20, 4, 99, &[]).unwrap();
    assert!(graph.node_ids().all(|id| graph.active_degree(id) == 4));
    assert_eq!(graph.edge_count(), 40);
}

#[test]
fn stable_store_rejects_dangling_duplicate_and_cyclic_state() {
    let mut graph = GraphStore::with_nodes(3);
    graph.add_edge(0, 1).unwrap();
    assert_eq!(graph.add_edge(1, 0), Err(GraphError::DuplicateEdge(0, 1)));
    assert_eq!(graph.add_edge(0, 3), Err(GraphError::DanglingNode(3)));
    assert_eq!(
        graph.set_tree(2, Some(1), vec![2, 1, 2]),
        Err(GraphError::InvalidAncestry(2))
    );
}

#[test]
fn attachment_selectors_are_deterministic() {
    let graph = GraphStore::generate(TopologyKind::Chain, 7, 2, 7, &[]).unwrap();
    assert_eq!(
        graph
            .select_attachment(AttachmentSelector::Leaf, 1, 1)
            .unwrap(),
        0
    );
    assert_eq!(
        graph
            .select_attachment(AttachmentSelector::Hub, 1, 1)
            .unwrap(),
        1
    );
    assert_eq!(
        graph
            .select_attachment(AttachmentSelector::Articulation, 1, 1)
            .unwrap(),
        1
    );
    assert_eq!(
        graph
            .select_attachment(AttachmentSelector::Random, 5, 2)
            .unwrap(),
        graph
            .select_attachment(AttachmentSelector::Random, 5, 2)
            .unwrap()
    );
}

#[test]
fn compact_footprint_is_published_by_formula() {
    let graph = GraphStore::generate(TopologyKind::Chain, 100, 2, 1, &[]).unwrap();
    let footprint = graph.memory_footprint();
    assert!(footprint.fixed_bytes_per_node <= 48);
    assert_eq!(footprint.fixed_bytes_per_edge, 8);
    assert!(footprint.allocated_bytes < 16_000);
}
