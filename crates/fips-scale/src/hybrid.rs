use crate::{
    CohortEngine, CohortReport, ScaleError, build_scale_artifact, exact_bloom_sample,
    translate_boundary,
};
use fips_artifact::{BloomFidelity, ReproductionBundle, RunArtifact, SampledRegion, ScaleFidelity};
use fips_engine::IndividualEngine;
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SamplingPolicy {
    RootSpine,
    BottleneckCut,
    SelectedSubtree,
    AnomalyDriven,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactRegionEvidence {
    pub region_id: String,
    pub source_cohort_id: String,
    pub policy: SamplingPolicy,
    pub requested_nodes: u64,
    pub instantiated_nodes: u64,
    pub artifact: RunArtifact,
    pub reproduction: ReproductionBundle,
    pub bloom: crate::ExactBloomSample,
    pub boundary: crate::BloomBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridReport {
    pub kind: String,
    pub cohort: CohortReport,
    pub exact: ExactRegionEvidence,
    pub aggregate_population: u64,
    pub exact_population: u64,
    pub reconciled_population: u64,
    pub causal_transition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridRun {
    pub artifact: RunArtifact,
    pub report: HybridReport,
}

#[derive(Debug, Clone, Default)]
pub struct HybridEngine;

impl HybridEngine {
    pub fn run(
        &self,
        plan: &NormalizedPlan,
        variant: &str,
        policy: SamplingPolicy,
        requested_nodes: u64,
    ) -> Result<HybridRun, HybridError> {
        let mut cohort = CohortEngine.run(plan, variant)?.report;
        let source_index = select_cohort(&cohort, policy);
        let source = cohort.cohorts[source_index].clone();
        let instantiated_nodes = requested_nodes.min(source.population).clamp(2, 128);
        let sample_plan = sample_plan(plan, instantiated_nodes, policy);
        let individual = IndividualEngine
            .run_plan(&sample_plan)
            .map_err(|error| HybridError::Engine(error.to_string()))?;
        let region_id = format!("sample-{}", source.id);
        let bloom = exact_bloom_sample(
            &region_id,
            source.population,
            instantiated_nodes,
            4096,
            3,
            plan.seed,
        );
        let boundary = translate_boundary(&source, &bloom);
        cohort.fidelity.scale = ScaleFidelity::Hybrid;
        cohort.fidelity.bloom = BloomFidelity::SampledExact;
        cohort.fidelity.sampled_regions = vec![SampledRegion {
            id: region_id.clone(),
            selection: format!("{policy:?}").to_lowercase(),
            node_count: instantiated_nodes,
        }];
        let report = HybridReport {
            kind: "hybrid-root-ratchet-report/v1alpha1".to_owned(),
            aggregate_population: plan_nodes(plan).saturating_sub(instantiated_nodes),
            exact_population: instantiated_nodes,
            reconciled_population: plan_nodes(plan),
            causal_transition: format!("cohort:{}->exact:{region_id}", source.id),
            exact: ExactRegionEvidence {
                region_id,
                source_cohort_id: source.id,
                policy,
                requested_nodes,
                instantiated_nodes,
                artifact: individual.artifact,
                reproduction: individual.reproduction,
                bloom,
                boundary,
            },
            cohort,
        };
        if report.aggregate_population + report.exact_population != report.reconciled_population
            || !report.exact.boundary.reconciles
        {
            return Err(HybridError::Reconciliation);
        }
        let mut artifact = build_scale_artifact(plan, &report.cohort)?;
        artifact.samples.push(serde_json::to_value(&report)?);
        artifact.manifest.fidelity = report.cohort.fidelity.clone();
        artifact.validate()?;
        Ok(HybridRun { artifact, report })
    }
}

fn select_cohort(report: &CohortReport, policy: SamplingPolicy) -> usize {
    match policy {
        SamplingPolicy::RootSpine => 0,
        SamplingPolicy::BottleneckCut => report.cohorts.len() / 2,
        SamplingPolicy::SelectedSubtree => report.cohorts.len().saturating_sub(1),
        SamplingPolicy::AnomalyDriven => report
            .cohorts
            .iter()
            .enumerate()
            .max_by_key(|(_, cohort)| cohort.key.depth_end.saturating_mul(cohort.population))
            .map(|(index, _)| index)
            .unwrap_or(0),
    }
}

fn sample_plan(source: &NormalizedPlan, nodes: u64, policy: SamplingPolicy) -> NormalizedPlan {
    let mut plan = source.clone();
    set(&mut plan.campaign, "/scale/nodes", Value::from(nodes));
    set(
        &mut plan.campaign,
        "/engine/modes",
        Value::String("compact-discrete-event".to_owned()),
    );
    set(
        &mut plan.campaign,
        "/topology/generator",
        Value::String(
            match policy {
                SamplingPolicy::RootSpine => "chain",
                _ => "balanced-tree",
            }
            .to_owned(),
        ),
    );
    set(
        &mut plan.campaign,
        "/transports/assignment",
        Value::String("all-udp".to_owned()),
    );
    let arrivals = plan
        .campaign
        .pointer("/identities/arrivals/count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(nodes - 1);
    set(
        &mut plan.campaign,
        "/identities/arrivals/count",
        Value::from(arrivals),
    );
    set(
        &mut plan.campaign,
        "/traffic/parameters/flow_count",
        Value::from(nodes.min(64)),
    );
    set(
        &mut plan.campaign,
        "/fidelity/billion_node_representation",
        Value::String("not-requested".to_owned()),
    );
    set(
        &mut plan.campaign,
        "/fidelity/protocol",
        Value::String("semantic-exact".to_owned()),
    );
    set(
        &mut plan.campaign,
        "/fidelity/bloom",
        Value::String("exact-bits".to_owned()),
    );
    set(
        &mut plan.campaign,
        "/fidelity/serialization",
        Value::String("executable-codec".to_owned()),
    );
    plan.axes.clear();
    plan.campaign_sha256 = format!("hybrid-sample-{}-{nodes}", source.campaign_sha256);
    plan
}

fn set(root: &mut Value, pointer: &str, value: Value) {
    if let Some(slot) = root.pointer_mut(pointer) {
        *slot = value;
    }
}
fn plan_nodes(plan: &NormalizedPlan) -> u64 {
    plan.campaign
        .pointer("/scale/nodes")
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

#[derive(Debug, Error)]
pub enum HybridError {
    #[error(transparent)]
    Scale(#[from] ScaleError),
    #[error("exact-region individual engine failed: {0}")]
    Engine(String),
    #[error(transparent)]
    Artifact(#[from] fips_artifact::ArtifactError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("hybrid exact and aggregate populations do not reconcile")]
    Reconciliation,
}
