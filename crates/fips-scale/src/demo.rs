use crate::{
    BASELINE_VARIANT, BLOOM_DELTA_VARIANT, CohortEngine, DAMPENING_VARIANT, Estimate, HybridEngine,
    HybridReport, SamplingPolicy,
};
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoScenario {
    pub topology: String,
    pub cadence_ns: u64,
    pub variant: String,
    pub represented_nodes: u64,
    pub allocated_cohorts: usize,
    pub maximum_depth: Estimate,
    pub control_bytes: Estimate,
    pub bloom_fpr_ppb: Estimate,
    pub peak_queue_bytes: Estimate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoResourceBudget {
    pub maximum_allocated_cohorts: usize,
    pub maximum_exact_nodes: u64,
    pub maximum_memory_bytes: u64,
    pub maximum_wall_time_seconds: u64,
    pub bounded_allocation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillionNodeDemo {
    pub kind: String,
    pub represented_nodes: u64,
    pub representation_claim: String,
    pub scenarios: Vec<DemoScenario>,
    pub minimum_control_bytes: String,
    pub maximum_control_bytes: String,
    pub exact_anomaly: HybridReport,
    pub resource_budget: DemoResourceBudget,
    pub headline_warning: String,
}

pub fn billion_node_demo(plan: &NormalizedPlan) -> Result<BillionNodeDemo, DemoError> {
    let mut scenarios = Vec::new();
    for topology in ["chain", "balanced-tree"] {
        for cadence_ns in [250_000_000_u64, 500_000_000, 1_000_000_000] {
            for variant in [BASELINE_VARIANT, DAMPENING_VARIANT, BLOOM_DELTA_VARIANT] {
                let scenario_plan = scenario_plan(plan, topology, cadence_ns);
                let run = CohortEngine.run(&scenario_plan, variant)?;
                scenarios.push(DemoScenario {
                    topology: topology.to_owned(),
                    cadence_ns,
                    variant: variant.to_owned(),
                    represented_nodes: run.report.represented_nodes,
                    allocated_cohorts: run.report.allocated_cohorts,
                    maximum_depth: run.report.metrics.maximum_depth,
                    control_bytes: run.report.metrics.control_bytes,
                    bloom_fpr_ppb: run.report.metrics.bloom_fpr_ppb,
                    peak_queue_bytes: run.report.metrics.peak_queue_bytes,
                });
            }
        }
    }
    let anomaly_plan = scenario_plan(plan, "balanced-tree", 500_000_000);
    let exact_anomaly = HybridEngine
        .run(
            &anomaly_plan,
            BASELINE_VARIANT,
            SamplingPolicy::AnomalyDriven,
            16,
        )?
        .report;
    let controls = scenarios
        .iter()
        .map(|scenario| scenario.control_bytes.value.parse::<u128>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DemoError::Metric)?;
    let maximum_allocated_cohorts = scenarios
        .iter()
        .map(|scenario| scenario.allocated_cohorts)
        .max()
        .unwrap_or(0);
    Ok(BillionNodeDemo {
        kind: "honest-billion-node-root-ratchet/v1alpha1".to_owned(),
        represented_nodes: 1_000_000_000,
        representation_claim: "analytical cohorts with one explicitly identified exact anomaly sample".to_owned(),
        minimum_control_bytes: controls.iter().min().copied().unwrap_or(0).to_string(),
        maximum_control_bytes: controls.iter().max().copied().unwrap_or(0).to_string(),
        scenarios,
        exact_anomaly,
        resource_budget: DemoResourceBudget {
            maximum_allocated_cohorts,
            maximum_exact_nodes: 16,
            maximum_memory_bytes: 64 * 1024 * 1024,
            maximum_wall_time_seconds: 30,
            bounded_allocation: true,
        },
        headline_warning: "No individual-node claim applies outside the 16-node exact sample; billion-node totals are bounded cohort estimates.".to_owned(),
    })
}

fn scenario_plan(source: &NormalizedPlan, topology: &str, cadence_ns: u64) -> NormalizedPlan {
    let mut plan = source.clone();
    set(
        &mut plan.campaign,
        "/scale/nodes",
        Value::from(1_000_000_000_u64),
    );
    set(
        &mut plan.campaign,
        "/topology/generator",
        Value::String(topology.to_owned()),
    );
    set(
        &mut plan.campaign,
        "/identities/arrivals/schedule/interval",
        serde_json::json!({"nanoseconds": cadence_ns}),
    );
    set(
        &mut plan.campaign,
        "/fidelity/billion_node_representation",
        Value::String("cohort-with-sampled-exact-regions".to_owned()),
    );
    plan.axes.clear();
    plan.campaign_sha256 = format!("billion-{}-{topology}-{cadence_ns}", source.campaign_sha256);
    plan
}

fn set(root: &mut Value, pointer: &str, value: Value) {
    if let Some(slot) = root.pointer_mut(pointer) {
        *slot = value;
    }
}

#[derive(Debug, Error)]
pub enum DemoError {
    #[error(transparent)]
    Scale(#[from] crate::ScaleError),
    #[error(transparent)]
    Hybrid(#[from] crate::HybridError),
    #[error("invalid decimal metric in demo scenario")]
    Metric,
}
