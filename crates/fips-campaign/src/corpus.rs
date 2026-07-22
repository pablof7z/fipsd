//! Deterministic minimized-failure promotion and batch regression replay.

use crate::ShrinkResult;
use fips_artifact::{FidelityContract, ReproductionBundle};
use fips_engine::IndividualEngine;
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Whether a regression is model-only or confirmed against real daemons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DaemonConfirmation {
    ModelOnly,
    DaemonConfirmed,
}

/// Reviewed regression-corpus metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusMetadata {
    pub corpus_version: String,
    pub semantic_version_range: String,
    pub first_seen: String,
    pub minimized_from: String,
    pub daemon_confirmation: DaemonConfirmation,
    pub expected_assertions: Vec<String>,
    pub fidelity: FidelityContract,
    pub retirement: Option<String>,
}

/// One standalone corpus entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusEntry {
    pub id: String,
    pub metadata: CorpusMetadata,
    pub reproduction: ReproductionBundle,
}

/// Per-entry replay outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusReplayOutcome {
    pub id: String,
    pub outcome: String,
    pub daemon_confirmation: DaemonConfirmation,
    pub first_seen: String,
    pub minimized_from: String,
}

/// Batch corpus report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusReport {
    pub kind: String,
    pub outcomes: Vec<CorpusReplayOutcome>,
    pub passed: u64,
    pub failed: u64,
}

/// Build an entry from a shrink result with a standalone bundle.
pub fn corpus_entry(
    shrink: &ShrinkResult,
    first_seen: impl Into<String>,
    semantic_version_range: impl Into<String>,
    confirmation: DaemonConfirmation,
) -> Result<CorpusEntry, CorpusError> {
    let reproduction = shrink
        .reproduction
        .clone()
        .ok_or(CorpusError::MissingReproduction)?;
    let digest = Sha256::digest(
        reproduction
            .to_canonical_json()
            .map_err(|error| CorpusError::Artifact(error.to_string()))?,
    );
    Ok(CorpusEntry {
        id: format!("regression-{}", &hex::encode(digest)[..24]),
        metadata: CorpusMetadata {
            corpus_version: "1".to_owned(),
            semantic_version_range: semantic_version_range.into(),
            first_seen: first_seen.into(),
            minimized_from: shrink.source_case_id.clone(),
            daemon_confirmation: confirmation,
            expected_assertions: reproduction.expected_assertions.clone(),
            fidelity: reproduction.fidelity.clone(),
            retirement: None,
        },
        reproduction,
    })
}

/// Promote without silently changing reviewed expectations.
pub fn promote(
    root: &Path,
    entry: &CorpusEntry,
    allow_expectation_update: bool,
) -> Result<PathBuf, CorpusError> {
    let directory = root.join(&entry.id);
    let metadata_path = directory.join("metadata.json");
    if metadata_path.exists() {
        let existing: CorpusMetadata = serde_json::from_slice(&fs::read(&metadata_path)?)?;
        if existing.expected_assertions != entry.metadata.expected_assertions
            && !allow_expectation_update
        {
            return Err(CorpusError::ExpectationChange(entry.id.clone()));
        }
    }
    fs::create_dir_all(&directory)?;
    write_json(&metadata_path, &entry.metadata)?;
    fs::write(
        directory.join("reproduction.json"),
        entry
            .reproduction
            .to_canonical_json()
            .map_err(|error| CorpusError::Artifact(error.to_string()))?,
    )?;
    Ok(directory)
}

/// Load every active entry in stable ID order.
pub fn load_corpus(root: &Path) -> Result<Vec<CorpusEntry>, CorpusError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut directories = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    directories.sort_by_key(|entry| entry.file_name());
    let mut entries = Vec::new();
    for directory in directories {
        let id = directory.file_name().to_string_lossy().into_owned();
        let metadata: CorpusMetadata =
            serde_json::from_slice(&fs::read(directory.path().join("metadata.json"))?)?;
        if metadata.retirement.is_some() {
            continue;
        }
        let reproduction: ReproductionBundle =
            serde_json::from_slice(&fs::read(directory.path().join("reproduction.json"))?)?;
        entries.push(CorpusEntry {
            id,
            metadata,
            reproduction,
        });
    }
    Ok(entries)
}

/// Replay the active corpus at each entry's declared fidelity.
pub fn replay_corpus(root: &Path) -> Result<CorpusReport, CorpusError> {
    let mut outcomes = Vec::new();
    let mut passed = 0_u64;
    for entry in load_corpus(root)? {
        let plan: NormalizedPlan =
            serde_json::from_value(entry.reproduction.normalized_plan.clone())?;
        let outcome = match IndividualEngine.run_plan(&plan) {
            Ok(run) => {
                let satisfied = entry.metadata.expected_assertions.iter().all(|expected| {
                    run.artifact
                        .assertion_results
                        .iter()
                        .any(|assertion| assertion.id == *expected && assertion.outcome == "pass")
                });
                if satisfied {
                    passed += 1;
                    "pass".to_owned()
                } else {
                    "expectation-mismatch".to_owned()
                }
            }
            Err(error) => format!("engine-failure:{error}"),
        };
        outcomes.push(CorpusReplayOutcome {
            id: entry.id,
            outcome,
            daemon_confirmation: entry.metadata.daemon_confirmation,
            first_seen: entry.metadata.first_seen,
            minimized_from: entry.metadata.minimized_from,
        });
    }
    Ok(CorpusReport {
        kind: "regression-corpus-report/v1alpha1".to_owned(),
        failed: outcomes.len() as u64 - passed,
        passed,
        outcomes,
    })
}

/// Corpus workflow failure.
#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("shrink result has no standalone reproduction bundle")]
    MissingReproduction,
    #[error("expectations changed for {0}; pass the explicit reviewed update flag")]
    ExpectationChange(String),
    #[error("artifact serialization failed: {0}")]
    Artifact(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), CorpusError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}
