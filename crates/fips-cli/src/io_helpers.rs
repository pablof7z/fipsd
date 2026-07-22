use anyhow::{Context, Result};
use fips_artifact::{LedgerEntry, RunArtifact};
use fips_engine::{RecoveryReport, RootRatchetReport};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn embedded_report(artifact: &RunArtifact) -> Result<RootRatchetReport> {
    artifact
        .samples
        .iter()
        .find_map(|sample| serde_json::from_value::<RootRatchetReport>(sample.clone()).ok())
        .context("artifact does not contain a root-ratchet report")
}

pub fn embedded_recovery_report(artifact: &RunArtifact) -> Option<RecoveryReport> {
    artifact
        .samples
        .iter()
        .find_map(|sample| serde_json::from_value::<RecoveryReport>(sample.clone()).ok())
}

pub fn causal_subtree<'a>(ledger: &'a [LedgerEntry], root: &str) -> Vec<&'a LedgerEntry> {
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

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes(path, &bytes)
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
