use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryAxis {
    pub name: String,
    pub values: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelitySupport {
    pub supported: Vec<String>,
    pub unsupported: Vec<String>,
    pub calibration: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub maximum_individual_nodes: u64,
    pub maximum_events: u64,
    pub maximum_memory_bytes: u64,
    pub maximum_wall_time_seconds: u64,
    pub larger_scale_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyContract {
    pub id: String,
    pub title: String,
    pub campaign_source: String,
    pub normative_schema: String,
    pub dimensions: Vec<String>,
    pub boundary_matrix: Vec<BoundaryAxis>,
    pub assertions: Vec<String>,
    pub report_recipe: Vec<String>,
    pub resource_budget: ResourceBudget,
    pub fidelity: FidelitySupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricEvidence {
    pub name: String,
    pub value: String,
    pub unit: String,
    pub fidelity: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryEvidence {
    pub id: String,
    pub below: MetricEvidence,
    pub at: MetricEvidence,
    pub above: MetricEvidence,
    pub conclusion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantEvidence {
    pub baseline: MetricEvidence,
    pub variant_id: String,
    pub variant: MetricEvidence,
    pub relative_change_ppm: i64,
    pub causal_explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionEvidence {
    pub id: String,
    pub outcome: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSupport {
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReproductionCase {
    pub id: String,
    pub normalized_plan: NormalizedPlan,
    pub selector_overrides: BTreeMap<String, serde_json::Value>,
    pub expected_assertion: String,
    pub oracle: OracleSupport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignQualification {
    pub contract: FamilyContract,
    pub baseline: Vec<MetricEvidence>,
    pub discovered_boundary: BoundaryEvidence,
    pub variant_comparison: VariantEvidence,
    pub assertions: Vec<AssertionEvidence>,
    pub minimized_reproduction: ReproductionCase,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtlasReport {
    pub api_version: String,
    pub atlas_id: String,
    pub fips_commit: String,
    pub generated_by: String,
    pub families: Vec<CampaignQualification>,
    pub family_count: usize,
    pub all_contracts_complete: bool,
    pub accounting_note: String,
}
