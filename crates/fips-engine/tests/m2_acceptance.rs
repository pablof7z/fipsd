use fips_artifact::LedgerEntry;
use fips_engine::{IndividualEngine, RecoveryReport};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_accepts(path: &Path, document: &Value) {
    let schema: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator
        .iter_errors(document)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
}

fn pretty(value: &impl serde::Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn descendants<'a>(ledger: &'a [LedgerEntry], root: &str) -> Vec<&'a LedgerEntry> {
    let mut ids = BTreeSet::from([root.to_owned()]);
    loop {
        let before = ids.len();
        for entry in ledger {
            if entry
                .causal_parent
                .as_ref()
                .is_some_and(|parent| ids.contains(parent))
            {
                ids.insert(entry.causal_id.clone());
            }
        }
        if ids.len() == before {
            break;
        }
    }
    ledger
        .iter()
        .filter(|entry| ids.contains(&entry.causal_id))
        .collect()
}

#[test]
fn checked_m2_recovery_replays_and_reconciles() {
    let repository = repository();
    let plan =
        fips_model::normalize_path(&repository.join("examples/m2/root-ratchet-recovery.yaml"))
            .unwrap();
    let first = IndividualEngine.run_plan(&plan).unwrap();
    let second = IndividualEngine.run_plan(&plan).unwrap();
    let report = first.recovery_report.as_ref().unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.artifact.to_canonical_json().unwrap(),
        fs::read(repository.join("fixtures/m2/root-ratchet-recovery-artifact.json")).unwrap()
    );
    assert_eq!(
        first.reproduction.to_canonical_json().unwrap(),
        fs::read(repository.join("fixtures/m2/root-ratchet-recovery-reproduction.json")).unwrap()
    );
    assert_eq!(
        pretty(report),
        fs::read(repository.join("fixtures/m2/root-ratchet-recovery-report.json")).unwrap()
    );
    schema_accepts(
        &repository.join("schemas/run-artifact-v1alpha1.schema.json"),
        &serde_json::to_value(&first.artifact).unwrap(),
    );
    schema_accepts(
        &repository.join("schemas/reproduction-bundle-v1alpha1.schema.json"),
        &serde_json::to_value(&first.reproduction).unwrap(),
    );
    assert!(report.costs.frames_reconcile);
    assert!(report.costs.projections_reconcile);
    assert!(report.costs.ledger_reconcile);
    assert!(report.assertions.iter().all(|item| item.outcome == "pass"));
}

#[test]
fn recovery_report_exposes_each_required_causal_surface() {
    let document =
        fs::read(repository().join("fixtures/m2/root-ratchet-recovery-report.json")).unwrap();
    let report: RecoveryReport = serde_json::from_slice(&document).unwrap();
    assert_eq!(report.bloom.sent, 88);
    assert_eq!(
        report.bloom.sent,
        report
            .per_arrival
            .iter()
            .map(|item| item.bloom_frames)
            .sum::<u64>()
    );
    assert!(report.cache.invalidations > 0 && report.cache.warmup_bytes > 0);
    assert!(report.lookup.lookups > 0 && !report.lookup.signals.is_empty());
    assert!(report.traffic.delivered_useful_bytes > 0);
    assert!(report.resources.maximum_queue_wait_ns > 0);
    assert!(report.markers.throughput_ns > report.markers.root_ns);
    assert_eq!(report.critical_path.component, "root-convergence");

    let tree = descendants(&report.causal_ledger, "input:arrival-0000");
    let stages = tree
        .iter()
        .map(|entry| entry.stage.as_str())
        .collect::<BTreeSet<_>>();
    for stage in [
        "performed",
        "state-mutated",
        "compute",
        "payload",
        "fsp",
        "fmp",
        "transport",
        "network",
        "useful-payload",
        "time",
    ] {
        assert!(stages.contains(stage), "missing causal stage {stage}");
    }
}
