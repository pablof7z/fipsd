//! Deterministic objective search, Pareto ranking, and resumable checkpoints.

use crate::{ExperimentCase, PlanManifest};
use fips_artifact::{ReproductionBundle, RunArtifact};
use fips_engine::IndividualEngine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

/// Search metric direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObjectiveDirection {
    Maximize,
    Minimize,
}

/// One metric objective.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveSpec {
    pub metric: String,
    pub direction: ObjectiveDirection,
}

/// Metric constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricConstraint {
    pub metric: String,
    pub maximum: Option<u64>,
    pub minimum: Option<u64>,
}

/// Bounded deterministic search request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub objectives: Vec<ObjectiveSpec>,
    pub constraints: Vec<MetricConstraint>,
    pub maximum_evaluations: usize,
    pub maximum_attacker_operations: Option<u64>,
}

/// One case evaluation with full replay evidence when successful.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseEvaluation {
    pub case_id: String,
    pub protocol_valid: bool,
    pub attacker_operations: u64,
    pub metrics: BTreeMap<String, u64>,
    pub outcome: String,
    pub artifact_sha256: Option<String>,
    pub artifact: Option<RunArtifact>,
    pub reproduction: Option<ReproductionBundle>,
}

/// Persistable search state. Existing evaluations are immutable on resume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchCheckpoint {
    pub kind: String,
    pub campaign_sha256: String,
    pub request: SearchRequest,
    pub evaluated: BTreeMap<String, CaseEvaluation>,
    pub complete: bool,
}

/// Ranked search output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub checkpoint: SearchCheckpoint,
    pub best_case_ids: Vec<String>,
    pub pareto_case_ids: Vec<String>,
}

/// Deterministic baseline optimizer.
#[derive(Debug, Clone, Default)]
pub struct SearchEngine;

impl SearchEngine {
    /// Evaluate planned cases in stable case-ID order and resume existing state.
    pub fn search(
        &self,
        manifest: &PlanManifest,
        request: SearchRequest,
        prior: Option<SearchCheckpoint>,
    ) -> Result<SearchResult, SearchError> {
        let mut checkpoint = match prior {
            Some(checkpoint) => {
                if checkpoint.campaign_sha256 != manifest.campaign_sha256
                    || checkpoint.request != request
                {
                    return Err(SearchError::CheckpointMismatch);
                }
                checkpoint
            }
            None => SearchCheckpoint {
                kind: "campaign-search-checkpoint/v1alpha1".to_owned(),
                campaign_sha256: manifest.campaign_sha256.clone(),
                request: request.clone(),
                evaluated: BTreeMap::new(),
                complete: false,
            },
        };
        let mut cases = manifest.cases.iter().collect::<Vec<_>>();
        cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        for case in cases {
            if checkpoint.evaluated.len() >= request.maximum_evaluations {
                break;
            }
            if checkpoint.evaluated.contains_key(&case.case_id) {
                continue;
            }
            let evaluation = evaluate(case, &request)?;
            checkpoint
                .evaluated
                .insert(case.case_id.clone(), evaluation);
        }
        checkpoint.complete =
            checkpoint.evaluated.len() == manifest.cases.len().min(request.maximum_evaluations);
        let eligible = checkpoint
            .evaluated
            .values()
            .filter(|evaluation| eligible(evaluation, &request))
            .collect::<Vec<_>>();
        let mut best_case_ids = Vec::new();
        for objective in &request.objectives {
            if let Some(case_id) = eligible
                .iter()
                .copied()
                .max_by(|left, right| compare(left, right, objective))
                .map(|evaluation| evaluation.case_id.clone())
            {
                if !best_case_ids.contains(&case_id) {
                    best_case_ids.push(case_id);
                }
            }
        }
        let pareto_case_ids = eligible
            .iter()
            .filter(|candidate| {
                !eligible.iter().any(|other| {
                    other.case_id != candidate.case_id
                        && dominates(other, candidate, &request.objectives)
                })
            })
            .map(|evaluation| evaluation.case_id.clone())
            .collect();
        Ok(SearchResult {
            checkpoint,
            best_case_ids,
            pareto_case_ids,
        })
    }
}

/// Search failure.
#[derive(Debug, Error)]
pub enum SearchError {
    #[error("search checkpoint belongs to a different campaign or request")]
    CheckpointMismatch,
    #[error("cannot serialize search evidence: {0}")]
    Json(#[from] serde_json::Error),
}

fn evaluate(case: &ExperimentCase, request: &SearchRequest) -> Result<CaseEvaluation, SearchError> {
    let attacker_operations = case
        .plan
        .campaign
        .pointer("/identities/arrivals/attacker_budget/operations")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let protocol_valid = case
        .plan
        .campaign
        .pointer("/adversaries/mode")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|mode| matches!(mode, "none" | "authenticated-protocol-valid"))
        && request
            .maximum_attacker_operations
            .is_none_or(|maximum| attacker_operations <= maximum);
    if !protocol_valid {
        return Ok(CaseEvaluation {
            case_id: case.case_id.clone(),
            protocol_valid: false,
            attacker_operations,
            metrics: BTreeMap::new(),
            outcome: "rejected-by-search-constraint".to_owned(),
            artifact_sha256: None,
            artifact: None,
            reproduction: None,
        });
    }
    match IndividualEngine.run_plan(&case.plan) {
        Ok(run) => {
            let mut metrics = BTreeMap::new();
            metrics.insert("convergence-time-ns".to_owned(), run.report.quiescence_ns);
            metrics.insert(
                "control-bytes".to_owned(),
                run.report.tree_announce.transmitted_frame_bytes,
            );
            metrics.insert(
                "parent-transitions".to_owned(),
                run.report.parent_transitions,
            );
            if let Some(recovery) = &run.recovery_report {
                metrics.insert(
                    "amplification-ppm".to_owned(),
                    recovery.costs.amplification_ppm,
                );
                metrics.insert("peak-queue-bytes".to_owned(), recovery.peak_queue_bytes);
                metrics.insert(
                    "goodput-stall-ns".to_owned(),
                    recovery.traffic.goodput_stall_ns,
                );
                metrics.insert("starved-flows".to_owned(), recovery.traffic.starved_flows);
                metrics.insert(
                    "cache-invalidations".to_owned(),
                    recovery.cache.invalidations,
                );
            }
            let bytes = run
                .artifact
                .to_canonical_json()
                .map_err(|error| serde_json::Error::io(std::io::Error::other(error.to_string())))?;
            Ok(CaseEvaluation {
                case_id: case.case_id.clone(),
                protocol_valid,
                attacker_operations,
                metrics,
                outcome: "success".to_owned(),
                artifact_sha256: Some(hex::encode(Sha256::digest(bytes))),
                artifact: Some(run.artifact),
                reproduction: Some(run.reproduction),
            })
        }
        Err(error) => Ok(CaseEvaluation {
            case_id: case.case_id.clone(),
            protocol_valid,
            attacker_operations,
            metrics: BTreeMap::new(),
            outcome: format!("engine-failure:{error}"),
            artifact_sha256: None,
            artifact: None,
            reproduction: None,
        }),
    }
}

fn eligible(evaluation: &CaseEvaluation, request: &SearchRequest) -> bool {
    evaluation.protocol_valid
        && evaluation.outcome == "success"
        && request.constraints.iter().all(|constraint| {
            let value = evaluation.metrics.get(&constraint.metric).copied();
            value.is_some_and(|value| {
                constraint.minimum.is_none_or(|minimum| value >= minimum)
                    && constraint.maximum.is_none_or(|maximum| value <= maximum)
            })
        })
}

fn compare(
    left: &CaseEvaluation,
    right: &CaseEvaluation,
    objective: &ObjectiveSpec,
) -> std::cmp::Ordering {
    let left_value = left.metrics.get(&objective.metric).copied().unwrap_or(0);
    let right_value = right.metrics.get(&objective.metric).copied().unwrap_or(0);
    let order = left_value.cmp(&right_value);
    match objective.direction {
        ObjectiveDirection::Maximize => order,
        ObjectiveDirection::Minimize => order.reverse(),
    }
    .then_with(|| right.case_id.cmp(&left.case_id))
}

fn dominates(left: &CaseEvaluation, right: &CaseEvaluation, objectives: &[ObjectiveSpec]) -> bool {
    let mut strict = false;
    objectives.iter().all(|objective| {
        let left_value = left.metrics.get(&objective.metric).copied().unwrap_or(0);
        let right_value = right.metrics.get(&objective.metric).copied().unwrap_or(0);
        let better = match objective.direction {
            ObjectiveDirection::Maximize => left_value >= right_value,
            ObjectiveDirection::Minimize => left_value <= right_value,
        };
        strict |= left_value != right_value;
        better
    }) && strict
}
