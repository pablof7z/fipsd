use fips_oracle::{
    ComparableEvidence, ComparableFrame, ComparableTransition, DaemonEvidence, FuzzOutcome,
    RecordedBackend, TELEMETRY_ADAPTER_VERSION, TelemetryInput, adapt_fuzz_result,
    compare_evidence, compile_to_chaos, default_oracle_suites, fixture_provenance,
    import_chaos_yaml, normalize_telemetry, run_oracle, to_yaml,
};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("fixtures/m5/generated"));
    fs::create_dir_all(&output).unwrap();
    let source = fs::read("fixtures/m5/chaos/smoke.yaml").unwrap();
    let imported = import_chaos_yaml("fixtures/m5/chaos/smoke.yaml", &source).unwrap();
    let bundle = compile_to_chaos(&imported.plan).unwrap();
    write_json(&output.join("imported-smoke.json"), &imported);
    write_json(&output.join("compiled-smoke-manifest.json"), &bundle);
    fs::write(
        output.join("compiled-smoke.yaml"),
        to_yaml(&bundle).unwrap(),
    )
    .unwrap();

    let telemetry = sample_telemetry();
    write_json(&output.join("normalized-telemetry.json"), &telemetry);
    let provenance = fixture_provenance(
        b"recorded-pinned-fips-binary",
        "sha256:recorded-pinned-image",
        &BTreeMap::from([("n01.yaml".to_owned(), b"public-config".to_vec())]),
    );
    let model = evidence("root-a", "executable-codec", "frame-a");
    let daemon = evidence("root-b", "captured-wire", "frame-b");
    write_json(
        &output.join("differential.json"),
        &compare_evidence(&model, &daemon, &telemetry, &provenance).unwrap(),
    );

    let exact = evidence("pass", "none", "");
    let recorded = DaemonEvidence {
        kind: "recorded-fixture-daemon-evidence/v1alpha1".to_owned(),
        comparable: exact.clone(),
        telemetry: empty_telemetry(),
        provenance,
        raw_output_sha256: "recorded-fixture-output".to_owned(),
        exit_code: 0,
    };
    let backend = RecordedBackend {
        id: "recorded-fixture-not-live-daemon".to_owned(),
        evidence: vec![recorded],
    };
    write_json(
        &output.join("recorded-oracle-report.json"),
        &run_oracle(&imported.plan, &exact, &backend, 3, 800_000).unwrap(),
    );
    write_json(
        &output.join("fuzz-crash.json"),
        &adapt_fuzz_result(
            "cargo-fuzz",
            FuzzOutcome::Crash,
            &[0xff, 0x00],
            "corpus-v1",
            42,
        ),
    );
    write_json(&output.join("suites.json"), &default_oracle_suites());
}

fn sample_telemetry() -> fips_oracle::NormalizedTelemetry {
    normalize_telemetry(TelemetryInput {
        adapter_version: TELEMETRY_ADAPTER_VERSION.to_owned(),
        sources: vec![
            fips_oracle::RawTelemetrySource {
                id: "tree:n01".to_owned(),
                kind: "control-snapshot".to_owned(),
                captured_at_ns: 2_000_000_000,
                payload: json!({"node_id":"n01","root":"root-a","parent":null,"ancestry":["n01"],"bloom":{"occupancy_ppb":100},"cache":{"entries":2},"sessions":{"count":1},"stats":{"lookup":{"count":3},"queue_bytes":20},"mmp":{"signals":4}}),
            },
            fips_oracle::RawTelemetrySource {
                id: "iperf:0".to_owned(),
                kind: "iperf-json".to_owned(),
                captured_at_ns: 3_000_000_000,
                payload: json!({"end":{"sum_received":{"bytes":4096}}}),
            },
            fips_oracle::RawTelemetrySource {
                id: "frame:0".to_owned(),
                kind: "frame-capture".to_owned(),
                captured_at_ns: 2_500_000_000,
                payload: json!({"id":"frame-0","sha256":"frame-b","size_bytes":168,"codec_commit":fips_oracle::PINNED_FIPS_COMMIT}),
            },
        ],
        clock_offset_ns: 20_000,
        clock_uncertainty_ns: 2_000_000,
    })
    .unwrap()
}

fn empty_telemetry() -> fips_oracle::NormalizedTelemetry {
    normalize_telemetry(TelemetryInput {
        adapter_version: TELEMETRY_ADAPTER_VERSION.to_owned(),
        sources: Vec::new(),
        clock_offset_ns: 0,
        clock_uncertainty_ns: 1_000_000,
    })
    .unwrap()
}

fn evidence(state: &str, frame_kind: &str, frame: &str) -> ComparableEvidence {
    ComparableEvidence {
        transitions: vec![ComparableTransition {
            ordinal: 0,
            kind: "root-agreement".to_owned(),
            node: "network".to_owned(),
            state: state.to_owned(),
            at_ns: 0,
            evidence: format!("evidence:{state}"),
        }],
        frames: (frame_kind != "none")
            .then(|| ComparableFrame {
                id: format!("frame:{frame}"),
                sha256: frame.to_owned(),
                size_bytes: 168,
                evidence_kind: frame_kind.to_owned(),
            })
            .into_iter()
            .collect(),
        metrics: BTreeMap::new(),
        unsupported_fields: BTreeSet::new(),
    }
}

fn write_json(path: &Path, value: &impl Serialize) {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    fs::write(path, bytes).unwrap();
}
