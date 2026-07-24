//! Exhaustive bounded exploration of authored input-action orderings.

use fips_engine::IndividualEngine;
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[path = "tiny_explorer_plan.rs"]
mod plan;
use plan::*;

pub const TINY_EXPLORATION_VERSION: &str =
    "experiments.fips.network/tiny-state-exploration/v1alpha1";
pub const TINY_COUNTEREXAMPLE_VERSION: &str =
    "experiments.fips.network/tiny-state-counterexample/v1alpha1";

const ACTIONS: &[&str] = &[
    "introduce-lower-root-node",
    "disappear-node",
    "reappear-node",
    "partition-network",
    "merge-network",
    "set-link-conditions",
    "restore-link-conditions",
    "synchronized-session-rekey",
    "expire-coordinate-cache",
    "simultaneous-lookups",
    "fail-transport-class",
    "restore-transport-class",
    "inject-parent-loop",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TinyExplorerConfig {
    pub maximum_nodes: u64,
    pub maximum_actions: usize,
    pub step_ns: u64,
}

impl Default for TinyExplorerConfig {
    fn default() -> Self {
        Self {
            maximum_nodes: 8,
            maximum_actions: 7,
            step_ns: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TinyCounterexample {
    pub api_version: String,
    pub id: String,
    pub fidelity: String,
    pub action_order: Vec<String>,
    pub normalized_plan: NormalizedPlan,
    pub failure: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TinyOutcome {
    pub sequence: u64,
    pub action_order: Vec<String>,
    pub terminal_signature: Option<String>,
    pub artifact_id: Option<String>,
    pub violation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TinyExplorationReport {
    pub api_version: String,
    pub exploration_id: String,
    pub fidelity: String,
    pub source_plan: NormalizedPlan,
    pub action_count: usize,
    pub expected_permutations: u64,
    pub explored_permutations: u64,
    pub exhaustive: bool,
    pub terminal_states: BTreeMap<String, u64>,
    pub outcomes: Vec<TinyOutcome>,
    pub counterexamples: Vec<TinyCounterexample>,
}

#[derive(Debug, Error)]
pub enum TinyExplorerError {
    #[error("tiny-state exploration requires a scalar individual-node plan")]
    Unresolved,
    #[error("tiny-state node count {actual} exceeds configured maximum {maximum}")]
    NodeLimit { actual: u64, maximum: u64 },
    #[error("tiny-state action count {actual} must be in 1..={maximum}")]
    ActionLimit { actual: usize, maximum: usize },
    #[error("tiny-state step must be positive")]
    ZeroStep,
    #[error("tiny-state plan is missing a Campaign events array")]
    MissingEvents,
    #[error("tiny-state arithmetic overflow")]
    Arithmetic,
    #[error("tiny-state report serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct TinyStateExplorer;

impl TinyStateExplorer {
    pub fn explore(
        &self,
        plan: &NormalizedPlan,
        config: TinyExplorerConfig,
    ) -> Result<TinyExplorationReport, TinyExplorerError> {
        if !plan.axes.is_empty() {
            return Err(TinyExplorerError::Unresolved);
        }
        let nodes = plan
            .campaign
            .pointer("/scale/nodes")
            .and_then(Value::as_u64)
            .ok_or(TinyExplorerError::Unresolved)?;
        if nodes > config.maximum_nodes {
            return Err(TinyExplorerError::NodeLimit {
                actual: nodes,
                maximum: config.maximum_nodes,
            });
        }
        if config.step_ns == 0 {
            return Err(TinyExplorerError::ZeroStep);
        }
        let events = plan
            .campaign
            .pointer("/events")
            .and_then(Value::as_array)
            .ok_or(TinyExplorerError::MissingEvents)?;
        let actions = events
            .iter()
            .filter(explorable)
            .cloned()
            .collect::<Vec<_>>();
        if actions.is_empty() || actions.len() > config.maximum_actions {
            return Err(TinyExplorerError::ActionLimit {
                actual: actions.len(),
                maximum: config.maximum_actions,
            });
        }
        let fixed = events
            .iter()
            .filter(|event| !explorable(event))
            .cloned()
            .collect::<Vec<_>>();
        let start_ns = actions
            .iter()
            .filter_map(event_time)
            .min()
            .unwrap_or(1_000_000_000);
        let mut orders = Vec::new();
        let mut indices = (0..actions.len()).collect::<Vec<_>>();
        permutations(&mut indices, 0, &mut orders);
        let expected = factorial(actions.len())?;
        let mut outcomes = Vec::with_capacity(orders.len());
        let mut counterexamples = Vec::new();
        let mut terminal_states = BTreeMap::new();
        for (sequence, order) in orders.iter().enumerate() {
            let candidate =
                candidate_plan(plan, &fixed, &actions, order, start_ns, config.step_ns)?;
            let names = order
                .iter()
                .map(|index| action_id(&actions[*index]))
                .collect::<Vec<_>>();
            match IndividualEngine.run_plan(&candidate) {
                Ok(run) => {
                    let failed = run
                        .artifact
                        .assertion_results
                        .iter()
                        .find(|assertion| assertion.outcome != "pass")
                        .map(|assertion| {
                            format!("assertion {}: {}", assertion.id, assertion.outcome)
                        });
                    let signature = terminal_signature(&run.report)?;
                    *terminal_states.entry(signature.clone()).or_default() += 1;
                    let artifact_id = run.artifact.manifest.artifact_id.clone();
                    outcomes.push(TinyOutcome {
                        sequence: sequence as u64,
                        action_order: names.clone(),
                        terminal_signature: Some(signature),
                        artifact_id: Some(artifact_id),
                        violation: failed.clone(),
                    });
                    if let Some(failure) = failed {
                        counterexamples.push(counterexample(&candidate, names, failure)?);
                    }
                }
                Err(error) => {
                    let failure = classify_error(error);
                    outcomes.push(TinyOutcome {
                        sequence: sequence as u64,
                        action_order: names.clone(),
                        terminal_signature: None,
                        artifact_id: None,
                        violation: Some(failure.clone()),
                    });
                    counterexamples.push(counterexample(&candidate, names, failure)?);
                }
            }
        }
        let mut report = TinyExplorationReport {
            api_version: TINY_EXPLORATION_VERSION.to_owned(), exploration_id: String::new(),
            fidelity: "exhaustive action-order enumeration; individual semantic engine; authored timing window"
                .to_owned(),
            source_plan: plan.clone(), action_count: actions.len(), expected_permutations: expected,
            explored_permutations: outcomes.len() as u64, exhaustive: outcomes.len() as u64 == expected,
            terminal_states, outcomes, counterexamples,
        };
        report.exploration_id = digest(&serde_json::to_vec(&report)?);
        Ok(report)
    }
}

fn explorable(event: &&Value) -> bool {
    event
        .get("action")
        .and_then(Value::as_str)
        .is_some_and(|action| ACTIONS.contains(&action))
}

fn classify_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
