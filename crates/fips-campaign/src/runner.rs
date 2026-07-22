//! Parallel case execution, cancellation, checkpointing, budgets, and deduplication.

use crate::ExperimentCase;
use fips_artifact::{ReproductionBundle, RunArtifact};
use fips_engine::IndividualEngine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

/// Explicit campaign execution budgets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionBudgets {
    pub worker_count: usize,
    pub maximum_cases: usize,
    pub maximum_memory_bytes: u64,
    pub maximum_disk_bytes: u64,
}

impl Default for ExecutionBudgets {
    fn default() -> Self {
        Self {
            worker_count: 1,
            maximum_cases: usize::MAX,
            maximum_memory_bytes: u64::MAX,
            maximum_disk_bytes: u64::MAX,
        }
    }
}

/// Cooperative campaign cancellation.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Isolated case outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseOutcome {
    pub case_id: String,
    pub outcome: String,
    pub artifact_sha256: Option<String>,
    pub evidence_bytes: u64,
    pub artifact: Option<RunArtifact>,
    pub reproduction: Option<ReproductionBundle>,
}

/// Resumable completed-case manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignCheckpoint {
    pub kind: String,
    pub campaign_sha256: String,
    pub completed: BTreeMap<String, CaseOutcome>,
}

impl CampaignCheckpoint {
    /// Persist through a same-directory temporary file and atomic rename.
    pub fn save(&self, path: &Path) -> Result<(), RunnerError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(RunnerError::Io)?;
        }
        let temporary = path.with_extension("json.tmp");
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        fs::write(&temporary, bytes).map_err(RunnerError::Io)?;
        fs::rename(&temporary, path).map_err(RunnerError::Io)
    }

    pub fn load(path: &Path) -> Result<Self, RunnerError> {
        serde_json::from_slice(&fs::read(path).map_err(RunnerError::Io)?).map_err(Into::into)
    }
}

/// Valid complete or partial campaign report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignRunReport {
    pub kind: String,
    pub budgets: ExecutionBudgets,
    pub partial: bool,
    pub termination: Option<String>,
    pub completed_cases: usize,
    pub skipped_completed_cases: usize,
    pub evidence_bytes: u64,
    pub unique_artifacts: usize,
    pub checkpoint: CampaignCheckpoint,
}

/// Deterministic local worker runner.
#[derive(Debug, Clone, Default)]
pub struct CampaignRunner;

impl CampaignRunner {
    /// Execute unresolved cases in parallel and fold outcomes by case ID.
    pub fn run(
        &self,
        campaign_sha256: &str,
        cases: &[ExperimentCase],
        budgets: ExecutionBudgets,
        prior: Option<CampaignCheckpoint>,
        cancellation: CancellationToken,
    ) -> Result<CampaignRunReport, RunnerError> {
        let mut checkpoint = prior.unwrap_or_else(|| CampaignCheckpoint {
            kind: "campaign-checkpoint/v1alpha1".to_owned(),
            campaign_sha256: campaign_sha256.to_owned(),
            completed: BTreeMap::new(),
        });
        if checkpoint.campaign_sha256 != campaign_sha256 {
            return Err(RunnerError::CheckpointMismatch);
        }
        let skipped_completed_cases = cases
            .iter()
            .filter(|case| checkpoint.completed.contains_key(&case.case_id))
            .count();
        let remaining_slots = budgets
            .maximum_cases
            .saturating_sub(checkpoint.completed.len());
        let mut pending = cases
            .iter()
            .filter(|case| !checkpoint.completed.contains_key(&case.case_id))
            .take(remaining_slots)
            .cloned()
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        let mut outcomes = execute_parallel(pending, budgets.worker_count.max(1), &cancellation);
        outcomes.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        let mut evidence_bytes = checkpoint
            .completed
            .values()
            .map(|outcome| outcome.evidence_bytes)
            .sum::<u64>();
        let mut termination = None;
        for outcome in outcomes {
            if cancellation.is_cancelled() {
                termination = Some("cancelled".to_owned());
                break;
            }
            let projected = evidence_bytes.saturating_add(outcome.evidence_bytes);
            if projected > budgets.maximum_disk_bytes {
                termination = Some("disk-budget".to_owned());
                break;
            }
            if outcome.evidence_bytes > budgets.maximum_memory_bytes {
                termination = Some("memory-budget".to_owned());
                break;
            }
            evidence_bytes = projected;
            checkpoint
                .completed
                .insert(outcome.case_id.clone(), outcome);
        }
        if checkpoint.completed.len() >= budgets.maximum_cases
            && checkpoint.completed.len() < cases.len()
        {
            termination.get_or_insert_with(|| "case-count-budget".to_owned());
        }
        let unique_artifacts = checkpoint
            .completed
            .values()
            .filter_map(|outcome| outcome.artifact_sha256.clone())
            .collect::<BTreeSet<_>>()
            .len();
        let partial = checkpoint.completed.len() < cases.len();
        Ok(CampaignRunReport {
            kind: "campaign-run-report/v1alpha1".to_owned(),
            budgets,
            partial,
            termination,
            completed_cases: checkpoint.completed.len(),
            skipped_completed_cases,
            evidence_bytes,
            unique_artifacts,
            checkpoint,
        })
    }
}

/// Runner failure.
#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("campaign checkpoint belongs to a different campaign")]
    CheckpointMismatch,
    #[error("campaign checkpoint I/O failed: {0}")]
    Io(std::io::Error),
    #[error("campaign checkpoint JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

fn execute_parallel(
    cases: Vec<ExperimentCase>,
    workers: usize,
    cancellation: &CancellationToken,
) -> Vec<CaseOutcome> {
    if cases.is_empty() {
        return Vec::new();
    }
    let chunk_size = cases.len().div_ceil(workers);
    let mut outcomes = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in cases.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let cancellation = cancellation.clone();
            handles.push(scope.spawn(move || {
                chunk
                    .into_iter()
                    .take_while(|_| !cancellation.is_cancelled())
                    .map(run_isolated)
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            outcomes.extend(handle.join().unwrap_or_else(|_| Vec::new()));
        }
    });
    outcomes
}

fn run_isolated(case: ExperimentCase) -> CaseOutcome {
    let result = std::panic::catch_unwind(|| IndividualEngine.run_plan(&case.plan));
    match result {
        Ok(Ok(run)) => match run.artifact.to_canonical_json() {
            Ok(bytes) => CaseOutcome {
                case_id: case.case_id,
                outcome: "success".to_owned(),
                artifact_sha256: Some(hex::encode(Sha256::digest(&bytes))),
                evidence_bytes: bytes.len() as u64,
                artifact: Some(run.artifact),
                reproduction: Some(run.reproduction),
            },
            Err(error) => failed(case.case_id, format!("artifact:{error}")),
        },
        Ok(Err(error)) => failed(case.case_id, format!("engine:{error}")),
        Err(_) => failed(case.case_id, "panic".to_owned()),
    }
}

fn failed(case_id: String, outcome: String) -> CaseOutcome {
    CaseOutcome {
        case_id,
        outcome,
        artifact_sha256: None,
        evidence_bytes: 0,
        artifact: None,
        reproduction: None,
    }
}
