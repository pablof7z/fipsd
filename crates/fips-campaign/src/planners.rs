//! Cartesian, covering-array, and seeded Monte Carlo planning.

use crate::{CaseCompiler, CaseSelection, CompileError, ExperimentCase};
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Campaign planning strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanningMode {
    /// Every valid combination.
    Cartesian,
    /// Deterministic greedy t-wise covering array.
    Covering,
    /// Seeded samples with optional stratification.
    MonteCarlo,
}

/// Planner request with replay parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannerRequest {
    /// Strategy.
    pub mode: PlanningMode,
    /// Requested t for covering arrays.
    pub strength: usize,
    /// Monte Carlo run count.
    pub run_count: usize,
    /// Deterministic sampling seed.
    pub seed: u64,
    /// Ensure each axis value is visited early when possible.
    pub stratified: bool,
    /// Informational confidence target.
    pub confidence_target: Option<f64>,
    /// Informational stable stopping criterion.
    pub stopping_criterion: Option<String>,
}

impl Default for PlannerRequest {
    fn default() -> Self {
        Self {
            mode: PlanningMode::Cartesian,
            strength: 2,
            run_count: 100,
            seed: 0,
            stratified: true,
            confidence_target: None,
            stopping_criterion: None,
        }
    }
}

/// Replayable campaign plan and achieved coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanManifest {
    /// Manifest discriminator.
    pub kind: String,
    /// Planning request.
    pub request: PlannerRequest,
    /// Source campaign hash.
    pub campaign_sha256: String,
    /// Full unconstrained matrix size.
    pub total_combinations: u64,
    /// Compiled executable cases.
    pub cases: Vec<ExperimentCase>,
    /// Invalid/constraint-filtered candidate count.
    pub omitted_cases: u64,
    /// Requested interactions satisfied.
    pub covered_interactions: u64,
    /// Total requested interactions.
    pub total_interactions: u64,
}

/// Deterministic multi-strategy planner.
#[derive(Debug, Clone, Default)]
pub struct CampaignPlanner {
    compiler: CaseCompiler,
}

impl CampaignPlanner {
    /// Use an explicit normalized-case compiler.
    pub fn new(compiler: CaseCompiler) -> Self {
        Self { compiler }
    }

    /// Compile one replayable plan.
    pub fn plan(
        &self,
        source: &NormalizedPlan,
        request: PlannerRequest,
    ) -> Result<PlanManifest, PlannerError> {
        let total_combinations = self.compiler.matrix_size(source)?;
        let candidates = all_indices(source)?;
        let (selected, total_interactions) = match request.mode {
            PlanningMode::Cartesian => (candidates.clone(), candidates.len() as u64),
            PlanningMode::Covering => {
                let total = interaction_universe(source, request.strength)?.len() as u64;
                (
                    covering_indices(source, &candidates, request.strength)?,
                    total,
                )
            }
            PlanningMode::MonteCarlo => (
                monte_carlo_indices(source, request.run_count, request.seed, request.stratified),
                candidates.len() as u64,
            ),
        };
        let mut cases = BTreeMap::new();
        let mut omitted_cases = 0_u64;
        for indices in selected {
            let selection = selection(source, &indices);
            match self.compiler.compile(source, selection) {
                Ok(case) => {
                    cases.entry(case.case_id.clone()).or_insert(case);
                }
                Err(CompileError::Constraint { .. } | CompileError::Compatibility { .. }) => {
                    omitted_cases += 1;
                }
                Err(error) => return Err(error.into()),
            }
        }
        let cases = cases.into_values().collect::<Vec<_>>();
        let covered_interactions = match request.mode {
            PlanningMode::Covering => covered_count(source, &cases, request.strength)?,
            PlanningMode::Cartesian => cases.len() as u64,
            PlanningMode::MonteCarlo => cases.len() as u64,
        };
        Ok(PlanManifest {
            kind: "campaign-plan/v1alpha1".to_owned(),
            request,
            campaign_sha256: source.campaign_sha256.clone(),
            total_combinations,
            cases,
            omitted_cases,
            covered_interactions,
            total_interactions,
        })
    }
}

/// Planning failure.
#[derive(Debug, Error)]
pub enum PlannerError {
    /// Case compilation failed.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// Covering strength is invalid.
    #[error("covering strength {strength} must be in 1..={axes}")]
    Strength { strength: usize, axes: usize },
    /// Full candidate set is too large for greedy covering.
    #[error("covering candidate matrix {0} exceeds deterministic planner limit 100000")]
    CoveringMatrix(u64),
}

type Interaction = Vec<(usize, usize)>;

fn all_indices(plan: &NormalizedPlan) -> Result<Vec<Vec<usize>>, PlannerError> {
    let mut rows = vec![Vec::new()];
    for axis in &plan.axes {
        let mut next = Vec::new();
        for row in &rows {
            for index in 0..axis.values.len() {
                let mut child = row.clone();
                child.push(index);
                next.push(child);
            }
        }
        rows = next;
        if rows.len() > 100_000 {
            return Err(PlannerError::CoveringMatrix(rows.len() as u64));
        }
    }
    Ok(rows)
}

fn selection(plan: &NormalizedPlan, indices: &[usize]) -> CaseSelection {
    plan.axes
        .iter()
        .zip(indices)
        .map(|(axis, index)| (axis.path.clone(), axis.values[*index].clone()))
        .collect()
}

fn interaction_universe(
    plan: &NormalizedPlan,
    strength: usize,
) -> Result<BTreeSet<Interaction>, PlannerError> {
    if strength == 0 || strength > plan.axes.len() {
        return Err(PlannerError::Strength {
            strength,
            axes: plan.axes.len(),
        });
    }
    let mut axis_sets = Vec::new();
    combinations(
        plan.axes.len(),
        strength,
        0,
        &mut Vec::new(),
        &mut axis_sets,
    );
    let mut output = BTreeSet::new();
    for axes in axis_sets {
        let mut rows = vec![Vec::new()];
        for axis in axes {
            let mut next = Vec::new();
            for row in &rows {
                for value in 0..plan.axes[axis].values.len() {
                    let mut child = row.clone();
                    child.push((axis, value));
                    next.push(child);
                }
            }
            rows = next;
        }
        output.extend(rows);
    }
    Ok(output)
}

fn combinations(
    count: usize,
    needed: usize,
    start: usize,
    current: &mut Vec<usize>,
    output: &mut Vec<Vec<usize>>,
) {
    if current.len() == needed {
        output.push(current.clone());
        return;
    }
    for value in start..count {
        current.push(value);
        combinations(count, needed, value + 1, current, output);
        current.pop();
    }
}

fn row_interactions(row: &[usize], strength: usize) -> BTreeSet<Interaction> {
    let mut axis_sets = Vec::new();
    combinations(row.len(), strength, 0, &mut Vec::new(), &mut axis_sets);
    axis_sets
        .into_iter()
        .map(|axes| axes.into_iter().map(|axis| (axis, row[axis])).collect())
        .collect()
}

fn covering_indices(
    plan: &NormalizedPlan,
    candidates: &[Vec<usize>],
    strength: usize,
) -> Result<Vec<Vec<usize>>, PlannerError> {
    let mut uncovered = interaction_universe(plan, strength)?;
    let mut selected = Vec::new();
    while !uncovered.is_empty() {
        let best = candidates
            .iter()
            .map(|row| {
                let score = row_interactions(row, strength)
                    .intersection(&uncovered)
                    .count();
                (score, row)
            })
            .max_by(|(left_score, left), (right_score, right)| {
                left_score.cmp(right_score).then_with(|| right.cmp(left))
            })
            .map(|(_, row)| row.clone())
            .expect("non-empty axes have candidates");
        for interaction in row_interactions(&best, strength) {
            uncovered.remove(&interaction);
        }
        selected.push(best);
    }
    Ok(selected)
}

fn monte_carlo_indices(
    plan: &NormalizedPlan,
    runs: usize,
    seed: u64,
    stratified: bool,
) -> Vec<Vec<usize>> {
    (0..runs)
        .map(|ordinal| {
            plan.axes
                .iter()
                .enumerate()
                .map(|(axis, values)| {
                    if stratified && ordinal < values.values.len() {
                        (ordinal + axis) % values.values.len()
                    } else {
                        (draw(seed, ordinal as u64, axis as u64) as usize) % values.values.len()
                    }
                })
                .collect()
        })
        .collect()
}

fn covered_count(
    source: &NormalizedPlan,
    cases: &[ExperimentCase],
    strength: usize,
) -> Result<u64, PlannerError> {
    let universe = interaction_universe(source, strength)?;
    let covered = cases
        .iter()
        .flat_map(|case| {
            let row = source
                .axes
                .iter()
                .map(|axis| {
                    axis.values
                        .iter()
                        .position(|value| case.selections.get(&axis.path) == Some(value))
                        .expect("compiled selection belongs to axis")
                })
                .collect::<Vec<_>>();
            row_interactions(&row, strength)
        })
        .collect::<BTreeSet<_>>();
    Ok(covered.intersection(&universe).count() as u64)
}

fn draw(seed: u64, ordinal: u64, lane: u64) -> u64 {
    let mut value = seed ^ ordinal.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ lane.rotate_left(17);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value.wrapping_mul(0x94D0_49BB_1331_11EB) ^ (value >> 31)
}
