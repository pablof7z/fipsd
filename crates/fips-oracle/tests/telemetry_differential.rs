mod common;

use fips_oracle::{
    ComparableEvidence, ComparableFrame, ComparableTransition, DifferenceDisposition,
    ObservationStatus, OracleClassification, RawTelemetrySource, TELEMETRY_ADAPTER_VERSION,
    TelemetryInput, compare_evidence, fixture_provenance, normalize_telemetry,
    redact_public_bundle,
};
use std::collections::{BTreeMap, BTreeSet};

fn telemetry() -> fips_oracle::NormalizedTelemetry {
    normalize_telemetry(TelemetryInput {
        adapter_version: TELEMETRY_ADAPTER_VERSION.to_owned(),
        sources: vec![
            RawTelemetrySource {
                id: "tree:n01".to_owned(), kind: "control-snapshot".to_owned(), captured_at_ns: 2_000_000_000,
                payload: serde_json::json!({"node_id":"n01","root":"root-a","parent":null,"ancestry":["n01"],"bloom":{"occupancy_ppb":100},"stats":{"queue_bytes":20}}),
            },
            RawTelemetrySource {
                id: "iperf:0".to_owned(), kind: "iperf-json".to_owned(), captured_at_ns: 3_000_000_000,
                payload: serde_json::json!({"end":{"sum_received":{"bytes":4096}}}),
            },
            RawTelemetrySource {
                id: "frame:0".to_owned(), kind: "frame-capture".to_owned(), captured_at_ns: 2_500_000_000,
                payload: serde_json::json!({"id":"frame-0","sha256":"abc","size_bytes":168,"codec_commit":fips_oracle::PINNED_FIPS_COMMIT}),
            },
        ],
        clock_offset_ns: 20_000,
        clock_uncertainty_ns: 2_000_000,
    }).unwrap()
}

#[test]
fn telemetry_keeps_raw_pointers_and_represents_missing_values_as_unknown() {
    let normalized = telemetry();
    assert_eq!(normalized.nodes.len(), 1);
    let node = &normalized.nodes[0];
    assert_eq!(node.root.status, ObservationStatus::Observed);
    assert_eq!(node.root.raw_source.as_deref(), Some("tree:n01"));
    assert_eq!(node.cache_entries.status, ObservationStatus::Unknown);
    assert_eq!(node.cache_entries.value, None);
    assert_eq!(
        normalized.metrics["traffic.useful-payload-bytes"].status,
        ObservationStatus::Sampled
    );
    assert!(
        normalize_telemetry(TelemetryInput {
            adapter_version: "future-v2".to_owned(),
            sources: Vec::new(),
            clock_offset_ns: 0,
            clock_uncertainty_ns: 0
        })
        .unwrap_err()
        .to_string()
        .contains("version drift")
    );
}

#[test]
fn differential_finds_first_transition_and_never_calls_unobserved_data_a_match() {
    let model = evidence("root-a", "abc", 4096);
    let daemon = evidence("root-b", "def", 4096);
    let provenance = fixture_provenance(b"real-fips-binary", "sha256:image", &BTreeMap::new());
    let report = compare_evidence(&model, &daemon, &telemetry(), &provenance).unwrap();
    assert_eq!(report.first_divergent_transition, Some(0));
    assert_eq!(
        report.classification,
        OracleClassification::ImplementationBug
    );
    assert!(
        report
            .differences
            .iter()
            .any(|item| item.disposition == DifferenceDisposition::SemanticDivergence)
    );
    assert!(report.differences.iter().any(|item| item.disposition
        == DifferenceDisposition::FrameDivergence
        && item.message.contains("captured")));
    let mut unobserved = model.clone();
    unobserved
        .metrics
        .insert("cache.invalidations".to_owned(), 7);
    let report = compare_evidence(&unobserved, &model, &telemetry(), &provenance).unwrap();
    assert!(
        report
            .differences
            .iter()
            .any(|item| item.path.contains("cache.invalidations")
                && item.disposition == DifferenceDisposition::Unobservable)
    );
}

#[test]
fn provenance_requires_dirty_patch_digest_and_public_bundles_remove_secrets() {
    let mut provenance = fixture_provenance(b"binary", "sha256:image", &BTreeMap::new());
    provenance.git_dirty = true;
    assert!(provenance.validate_for_comparison().is_err());
    provenance.patch_sha256 = Some("patch-digest".to_owned());
    provenance.validate_for_comparison().unwrap();
    let mut value = serde_json::json!({"config":{"private_key":"nope","nested":{"token":"nope","public":"yes"}}});
    let removed = redact_public_bundle(&mut value);
    assert_eq!(removed.len(), 2);
    assert_eq!(
        value
            .pointer("/config/nested/public")
            .and_then(serde_json::Value::as_str),
        Some("yes")
    );
    assert!(value.pointer("/config/private_key").is_none());
}

#[test]
fn clock_uncertainty_tolerates_only_variance_inside_the_recorded_window() {
    let mut model = evidence("root-a", "abc", 4096);
    model.frames.clear();
    let mut daemon = model.clone();
    daemon.transitions[0].at_ns += 1_000_000;
    let provenance = fixture_provenance(b"binary", "sha256:image", &BTreeMap::new());
    let report = compare_evidence(&model, &daemon, &telemetry(), &provenance).unwrap();
    assert_eq!(
        report.classification,
        OracleClassification::ToleratedNondeterminism
    );
    daemon.transitions[0].at_ns += 10_000_000;
    let report = compare_evidence(&model, &daemon, &telemetry(), &provenance).unwrap();
    assert_eq!(report.first_divergent_transition, Some(0));
    assert_eq!(
        report.classification,
        OracleClassification::NondeterministicEnvironment
    );
}

fn evidence(root: &str, frame: &str, useful: u64) -> ComparableEvidence {
    ComparableEvidence {
        transitions: vec![ComparableTransition {
            ordinal: 0,
            kind: "root-adoption".to_owned(),
            node: "n01".to_owned(),
            state: root.to_owned(),
            at_ns: 1_000_000_000,
            evidence: format!("event:{root}"),
        }],
        frames: vec![ComparableFrame {
            id: format!("frame:{frame}"),
            sha256: frame.to_owned(),
            size_bytes: 168,
            evidence_kind: if root == "root-a" {
                "executable-codec"
            } else {
                "captured-wire"
            }
            .to_owned(),
        }],
        metrics: BTreeMap::from([("traffic.useful-payload-bytes".to_owned(), useful)]),
        unsupported_fields: BTreeSet::new(),
    }
}
