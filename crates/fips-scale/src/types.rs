use fips_artifact::{FidelityContract, RunArtifact};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable cohort dimensions. Ranges keep state bounded at structural scale.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CohortKey {
    pub depth_start: u64,
    pub depth_end: u64,
    pub degree: u32,
    pub transport: String,
    pub resource: String,
    pub region: String,
    pub protocol_state: String,
}

/// Conserved population mass in one analytical state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cohort {
    pub id: String,
    pub key: CohortKey,
    pub population: u64,
}

/// Decimal estimate with a bound/uncertainty contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Estimate {
    pub value: String,
    pub lower: String,
    pub upper: String,
    pub unit: String,
    pub method: String,
    pub assumptions: Vec<String>,
    pub validated_range: String,
    pub uncertainty: String,
}

impl Estimate {
    pub fn exact(value: u128, unit: &str, method: &str) -> Self {
        let value = value.to_string();
        Self {
            value: value.clone(),
            lower: value.clone(),
            upper: value,
            unit: unit.to_owned(),
            method: method.to_owned(),
            assumptions: Vec::new(),
            validated_range: "closed-form".to_owned(),
            uncertainty: "exact under stated cohort assumptions".to_owned(),
        }
    }

    pub fn bounded(value: u128, error_ppm: u64, unit: &str, method: &str) -> Self {
        let delta = value.saturating_mul(u128::from(error_ppm)) / 1_000_000;
        Self {
            value: value.to_string(),
            lower: value.saturating_sub(delta).to_string(),
            upper: value.saturating_add(delta).to_string(),
            unit: unit.to_owned(),
            method: method.to_owned(),
            assumptions: vec!["exchangeable nodes within each cohort".to_owned()],
            validated_range: "see calibration report".to_owned(),
            uncertainty: format!("deterministic bound +/- {error_ppm} ppm"),
        }
    }
}

/// Common projection shared by cohort and hybrid runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleMetrics {
    pub root_adoptions: Estimate,
    pub parent_transitions: Estimate,
    pub maximum_depth: Estimate,
    pub control_bytes: Estimate,
    pub bloom_fpr_ppb: Estimate,
    pub peak_queue_bytes: Estimate,
    pub useful_payload_bytes: Estimate,
    pub quiescence_ns: Estimate,
}

/// Analytical run report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CohortReport {
    pub kind: String,
    pub represented_nodes: u64,
    pub allocated_cohorts: usize,
    pub variant: VariantIdentity,
    pub fidelity: FidelityContract,
    pub cohorts: Vec<Cohort>,
    pub metrics: ScaleMetrics,
    pub operation_counts: BTreeMap<String, u128>,
    pub population_before: u64,
    pub population_after: u64,
    pub assertions: Vec<String>,
}

/// One immutable scale run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleRun {
    pub artifact: RunArtifact,
    pub report: CohortReport,
}

/// Versioned variant identity included in run hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantIdentity {
    pub id: String,
    pub version: String,
    pub parameter_sha256: String,
    pub experimental: bool,
}
