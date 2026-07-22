use fips_artifact::RunArtifact;
use fips_query::{
    EventQuery, ExportLimits, Representation, analyze, compare, export_static, query_events,
};
use std::fs;

fn artifact() -> RunArtifact {
    serde_json::from_slice(include_bytes!(
        "../../../fixtures/m2/root-ratchet-recovery-artifact.json"
    ))
    .expect("fixture")
}

#[test]
fn summary_preserves_fidelity_provenance_and_separate_quiescence() {
    let document = analyze(&artifact()).expect("analysis");
    assert_eq!(document.representation, Representation::ExactGraph);
    assert!(!document.provenance.engine_source_revision.is_empty());
    assert!(document.quiescence.root_ns.is_some());
    assert!(document.quiescence.bloom_ns.is_some());
    assert_ne!(document.quiescence.root_ns, document.quiescence.bloom_ns);
    assert!(
        document
            .metrics
            .iter()
            .all(|metric| metric.source.total > 0)
    );
    assert_eq!(document.network.mode, "exact");
    assert!(!document.network.exact_nodes.is_empty());
    assert!(!document.network.exact_edges.is_empty());
    assert!(!document.root_wave.points.is_empty());
    assert!(document.root_wave.final_consensus_root.is_some());
}

#[test]
fn bounded_query_is_deterministic_and_addressable() {
    let source = artifact();
    let query = EventQuery {
        maximum_results: 5,
        ..EventQuery::default()
    };
    let first = query_events(&source, &query).expect("query");
    let second = query_events(&source, &query).expect("query");
    assert_eq!(first, second);
    assert!(first.events.len() <= 5);
    assert_eq!(first.source.total, source.event_trace.len());
}

#[test]
fn comparison_exposes_first_semantic_divergence() {
    let left = artifact();
    let mut right = left.clone();
    right.event_trace[0].kind.push_str("-variant");
    let result = compare(&left, &right).expect("comparison");
    assert!(result.compatible);
    assert_eq!(
        result.first_semantic_divergence,
        Some(left.event_trace[0].event_id.clone())
    );
}

#[test]
fn static_export_redacts_secrets_and_host_paths() {
    let mut source = artifact();
    source.normalized_plan["private_key"] = serde_json::json!("do-not-export");
    source.normalized_plan["output"] = serde_json::json!("/Users/alice/private/run.json");
    let root = std::env::temp_dir().join(format!("fips-query-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp");
    }
    export_static(&source, &root, ExportLimits::default()).expect("export");
    let exported = fs::read_to_string(root.join("artifact.json")).expect("artifact");
    assert!(!exported.contains("do-not-export"));
    assert!(!exported.contains("/Users/alice"));
    assert!(exported.contains("[redacted]"));
    assert!(exported.contains("$HOME/private/run.json"));
    fs::remove_dir_all(root).expect("cleanup");
}
