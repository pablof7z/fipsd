use crate::{
    Cohort, CohortKey, CohortReport, Estimate, ScaleMetrics, ScaleRun, build_scale_artifact,
    cohort_bloom, resolve_variant,
};
use fips_artifact::{
    Approximation, BloomFidelity, ComputeFidelity, FidelityContract, ProtocolFidelity,
    ScaleFidelity, WireFidelity,
};
use fips_model::NormalizedPlan;
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub struct CohortEngine;

impl CohortEngine {
    pub fn run(&self, plan: &NormalizedPlan, variant_id: &str) -> Result<ScaleRun, ScaleError> {
        let nodes = scalar_u64(&plan.campaign, "/scale/nodes")?;
        if nodes < 2 {
            return Err(ScaleError::Unsupported(
                "cohort scale requires at least two nodes".to_owned(),
            ));
        }
        let topology = scalar_str(&plan.campaign, "/topology/generator")?;
        let degree = optional_u64(&plan.campaign, "/topology/average_degree").unwrap_or(2) as u32;
        let cohorts = build_cohorts(nodes, topology, degree)?;
        let population = cohorts.iter().map(|cohort| cohort.population).sum::<u64>();
        let parameters = plan
            .campaign
            .pointer("/protocol/parameters")
            .cloned()
            .unwrap_or(Value::Null);
        let variant = resolve_variant(variant_id, &parameters)?;
        if !variant.supports("cohort") {
            return Err(ScaleError::Unsupported(format!(
                "{variant_id} does not support cohort fidelity"
            )));
        }
        let arrivals = optional_u64(&plan.campaign, "/identities/arrivals/count").unwrap_or(0);
        let cadence = duration(&plan.campaign, "/identities/arrivals/schedule/interval")
            .unwrap_or(500_000_000);
        let depth_weight = cohorts
            .iter()
            .map(|cohort| {
                let midpoint =
                    (u128::from(cohort.key.depth_start) + u128::from(cohort.key.depth_end)) / 2;
                midpoint * u128::from(cohort.population)
            })
            .sum::<u128>();
        let full_tree_bytes = u128::from(nodes) * 168 + depth_weight * 32;
        let mut root_adoptions = 0_u128;
        let mut bloom_bytes = 0_u128;
        let mut last_adoption = 0_u64;
        for generation in 0..arrivals {
            let at = generation.saturating_add(1).saturating_mul(cadence);
            let decision = variant.decide(crate::VariantContext {
                root_generation: generation,
                since_last_root_ns: at.saturating_sub(last_adoption),
                full_bloom_bytes: u128::from(nodes) * 512,
                changed_bloom_bits: nodes.div_ceil(64),
            });
            if decision.adopt_root {
                root_adoptions += 1;
                last_adoption = at;
                bloom_bytes = bloom_bytes.saturating_add(decision.bloom_bytes);
            }
        }
        let bloom = cohort_bloom(&cohorts, 4096, 3);
        let maximum_fpr = bloom.iter().map(|item| item.fpr_ppb).max().unwrap_or(0);
        let maximum_depth = cohorts
            .iter()
            .map(|cohort| cohort.key.depth_end)
            .max()
            .unwrap_or(0);
        let control_bytes = root_adoptions
            .saturating_mul(full_tree_bytes)
            .saturating_add(bloom_bytes);
        let useful = u128::from(nodes).saturating_mul(512);
        let quiescence = u128::from(arrivals.saturating_mul(cadence))
            .saturating_add(u128::from(maximum_depth).saturating_mul(1_000_000));
        let fidelity = cohort_fidelity(nodes);
        let metrics = ScaleMetrics {
            root_adoptions: Estimate::exact(root_adoptions, "count", "variant-decision-count/v1"),
            parent_transitions: Estimate::bounded(
                root_adoptions.saturating_mul(u128::from(nodes.saturating_sub(1))),
                20_000,
                "count",
                "cohort-parent-transition-sum/v1",
            ),
            maximum_depth: Estimate::bounded(
                u128::from(maximum_depth),
                20_000,
                "edges",
                "cohort-depth-band/v1",
            ),
            control_bytes: Estimate::bounded(control_bytes, 30_000, "bytes", "cohort-frame-sum/v1"),
            bloom_fpr_ppb: Estimate::bounded(
                u128::from(maximum_fpr),
                25_000,
                "ppb",
                "bloom-independent-bit-occupancy/v1",
            ),
            peak_queue_bytes: Estimate::bounded(
                control_bytes.div_ceil(u128::from(nodes.max(1))),
                50_000,
                "bytes",
                "fluid-queue-upper-bound/v1",
            ),
            useful_payload_bytes: Estimate::exact(useful, "bytes", "configured-payload/v1"),
            quiescence_ns: Estimate::bounded(
                quiescence,
                50_000,
                "nanoseconds",
                "depth-cadence-bound/v1",
            ),
        };
        let operation_counts = operation_counts(nodes, root_adoptions, control_bytes);
        let report = CohortReport {
            kind: "cohort-root-ratchet-report/v1alpha1".to_owned(),
            represented_nodes: nodes,
            allocated_cohorts: cohorts.len(),
            variant: variant.identity(),
            fidelity,
            cohorts,
            metrics,
            operation_counts,
            population_before: nodes,
            population_after: population,
            assertions: vec![
                "population-mass-conserved".to_owned(),
                "bounds-declared".to_owned(),
            ],
        };
        if report.population_before != report.population_after {
            return Err(ScaleError::Population {
                before: nodes,
                after: population,
            });
        }
        let artifact = build_scale_artifact(plan, &report)?;
        Ok(ScaleRun { artifact, report })
    }
}

fn build_cohorts(nodes: u64, topology: &str, degree: u32) -> Result<Vec<Cohort>, ScaleError> {
    let bands = match topology {
        "chain" => 64_u64.min(nodes),
        "balanced-tree" => nodes.ilog2() as u64 + 1,
        "regional-mesh" | "random-regular" | "scale-free" | "small-world" => 16_u64.min(nodes),
        other => {
            return Err(ScaleError::Unsupported(format!(
                "unsupported cohort topology {other}"
            )));
        }
    };
    let base = nodes / bands;
    let extra = nodes % bands;
    let depth_span = nodes.div_ceil(bands);
    Ok((0..bands)
        .map(|band| Cohort {
            id: format!("cohort-{band:04}"),
            key: CohortKey {
                depth_start: band.saturating_mul(depth_span),
                depth_end: band
                    .saturating_add(1)
                    .saturating_mul(depth_span)
                    .min(nodes)
                    .saturating_sub(1),
                degree,
                transport: "configured-mixture".to_owned(),
                resource: "configured-distribution".to_owned(),
                region: format!("region-{:02}", band % 8),
                protocol_state: "root-adopted".to_owned(),
            },
            population: base + u64::from(band < extra),
        })
        .collect())
}

fn cohort_fidelity(nodes: u64) -> FidelityContract {
    FidelityContract {
        wire: WireFidelity::Modeled,
        protocol: ProtocolFidelity::Cohort,
        compute: ComputeFidelity::OperationCounted,
        scale: ScaleFidelity::Cohort,
        bloom: BloomFidelity::CohortFpr,
        represented_nodes: nodes,
        approximations: vec![Approximation {
            method: "bounded-depth-degree-cohorts/v1".to_owned(),
            parameters: BTreeMap::from([("maximum-cohorts".to_owned(), "64".to_owned())]),
            validated_range:
                "2..=1000000 matched against individual engine; larger scales unvalidated"
                    .to_owned(),
            uncertainty: "per-metric deterministic bounds and calibration warnings".to_owned(),
        }],
        sampled_regions: Vec::new(),
    }
}

fn operation_counts(nodes: u64, roots: u128, bytes: u128) -> BTreeMap<String, u128> {
    let population = u128::from(nodes);
    BTreeMap::from([
        ("schnorr-sign".to_owned(), roots),
        (
            "schnorr-verify".to_owned(),
            roots.saturating_mul(population),
        ),
        ("ecdh".to_owned(), population.saturating_sub(1)),
        ("sha256".to_owned(), roots.saturating_mul(population)),
        ("aead".to_owned(), bytes.div_ceil(1024)),
        (
            "bloom-hash".to_owned(),
            roots.saturating_mul(population).saturating_mul(3),
        ),
        ("bloom-or".to_owned(), roots.saturating_mul(population)),
        (
            "population-transition".to_owned(),
            roots.saturating_mul(population),
        ),
    ])
}

fn scalar_u64(value: &Value, pointer: &str) -> Result<u64, ScaleError> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| ScaleError::Unsupported(format!("{pointer} must be a scalar integer")))
}

fn scalar_str<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, ScaleError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| ScaleError::Unsupported(format!("{pointer} must be a scalar string")))
}

fn optional_u64(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}
fn duration(value: &Value, pointer: &str) -> Option<u64> {
    value
        .pointer(pointer)
        .and_then(|item| item.get("nanoseconds"))
        .and_then(Value::as_u64)
}

#[derive(Debug, Error)]
pub enum ScaleError {
    #[error("unsupported cohort request: {0}")]
    Unsupported(String),
    #[error("population mass changed from {before} to {after}")]
    Population { before: u64, after: u64 },
    #[error(transparent)]
    Variant(#[from] crate::VariantError),
    #[error(transparent)]
    Artifact(#[from] fips_artifact::ArtifactError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
