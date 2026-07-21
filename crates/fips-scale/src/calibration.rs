use crate::{BASELINE_VARIANT, CohortEngine, HybridEngine, SamplingPolicy};
use fips_engine::IndividualEngine;
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationSample {
    pub nodes: u64,
    pub topology: String,
    pub seed: u64,
    pub metric: String,
    pub individual: u128,
    pub cohort: u128,
    pub error_ppm: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDistribution {
    pub metric: String,
    pub errors_ppm: Vec<i128>,
    pub median_absolute_ppm: u128,
    pub p95_absolute_ppm: u128,
    pub maximum_absolute_ppm: u128,
    pub validated_range: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub kind: String,
    pub samples: Vec<CalibrationSample>,
    pub distributions: BTreeMap<String, ErrorDistribution>,
    pub hybrid_sample_replayed: bool,
    pub automatic_warnings: Vec<String>,
}

pub fn calibrate(plan: &NormalizedPlan) -> Result<CalibrationReport, CalibrationError> {
    let mut samples = Vec::new();
    for nodes in [8_u64, 12, 32, 64] {
        for topology in ["chain", "balanced-tree"] {
            let matched = matched_plan(plan, nodes, topology);
            let individual = IndividualEngine
                .run_plan(&matched)
                .map_err(|error| CalibrationError::Engine(error.to_string()))?;
            let cohort = CohortEngine.run(&matched, BASELINE_VARIANT)?;
            let recovery = individual.recovery_report.as_ref().ok_or_else(|| {
                CalibrationError::Engine("matched run emitted no recovery report".to_owned())
            })?;
            let pairs = [
                (
                    "root-adoptions",
                    u128::from(individual.report.arrivals),
                    parse(&cohort.report.metrics.root_adoptions.value)?,
                ),
                (
                    "maximum-depth",
                    u128::from(individual.report.maximum_depth),
                    parse(&cohort.report.metrics.maximum_depth.value)?,
                ),
                (
                    "control-bytes",
                    u128::from(individual.report.tree_announce.transmitted_frame_bytes),
                    parse(&cohort.report.metrics.control_bytes.value)?,
                ),
                (
                    "bloom-fpr-ppb",
                    u128::from(recovery.bloom_fpr_ppb),
                    parse(&cohort.report.metrics.bloom_fpr_ppb.value)?,
                ),
                (
                    "peak-queue-bytes",
                    u128::from(recovery.peak_queue_bytes),
                    parse(&cohort.report.metrics.peak_queue_bytes.value)?,
                ),
                (
                    "useful-payload-bytes",
                    u128::from(recovery.traffic.delivered_useful_bytes),
                    parse(&cohort.report.metrics.useful_payload_bytes.value)?,
                ),
                (
                    "quiescence-ns",
                    u128::from(individual.report.quiescence_ns),
                    parse(&cohort.report.metrics.quiescence_ns.value)?,
                ),
            ];
            for (metric, exact, estimate) in pairs {
                samples.push(CalibrationSample {
                    nodes,
                    topology: topology.to_owned(),
                    seed: plan.seed,
                    metric: metric.to_owned(),
                    individual: exact,
                    cohort: estimate,
                    error_ppm: relative_error(exact, estimate),
                });
            }
        }
    }
    let mut distributions = BTreeMap::new();
    for metric in [
        "root-adoptions",
        "maximum-depth",
        "control-bytes",
        "bloom-fpr-ppb",
        "peak-queue-bytes",
        "useful-payload-bytes",
        "quiescence-ns",
    ] {
        let errors = samples
            .iter()
            .filter(|sample| sample.metric == metric)
            .map(|sample| sample.error_ppm)
            .collect::<Vec<_>>();
        let mut absolute = errors
            .iter()
            .map(|error| error.unsigned_abs())
            .collect::<Vec<_>>();
        absolute.sort_unstable();
        let p95 = absolute
            .len()
            .saturating_mul(95)
            .div_ceil(100)
            .saturating_sub(1);
        let maximum = absolute.last().copied().unwrap_or(0);
        distributions.insert(
            metric.to_owned(),
            ErrorDistribution {
                metric: metric.to_owned(),
                errors_ppm: errors,
                median_absolute_ppm: absolute.get(absolute.len() / 2).copied().unwrap_or(0),
                p95_absolute_ppm: absolute.get(p95).copied().unwrap_or(0),
                maximum_absolute_ppm: maximum,
                validated_range: "8..=64 nodes; chain and balanced-tree; matched seed".to_owned(),
                warning: (maximum > 250_000).then(|| "outside calibrated 25% envelope".to_owned()),
            },
        );
    }
    let hybrid = HybridEngine.run(
        &matched_plan(plan, 64, "balanced-tree"),
        BASELINE_VARIANT,
        SamplingPolicy::AnomalyDriven,
        8,
    )?;
    let automatic_warnings = distributions
        .values()
        .filter_map(|distribution| {
            distribution
                .warning
                .as_ref()
                .map(|warning| format!("{}: {warning}", distribution.metric))
        })
        .collect();
    Ok(CalibrationReport {
        kind: "cross-fidelity-calibration/v1alpha1".to_owned(),
        samples,
        distributions,
        hybrid_sample_replayed: hybrid.report.exact.artifact.validate().is_ok(),
        automatic_warnings,
    })
}

fn matched_plan(source: &NormalizedPlan, nodes: u64, topology: &str) -> NormalizedPlan {
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
        Value::String(topology.to_owned()),
    );
    set(
        &mut plan.campaign,
        "/transports/assignment",
        Value::String("all-udp".to_owned()),
    );
    set(
        &mut plan.campaign,
        "/fidelity/billion_node_representation",
        Value::String("not-requested".to_owned()),
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
        "/fidelity/bloom",
        Value::String("exact-bits".to_owned()),
    );
    plan.axes.clear();
    plan.campaign_sha256 = format!("calibration-{}-{nodes}-{topology}", source.campaign_sha256);
    plan
}

fn set(root: &mut Value, pointer: &str, value: Value) {
    if let Some(slot) = root.pointer_mut(pointer) {
        *slot = value;
    }
}
fn parse(value: &str) -> Result<u128, CalibrationError> {
    value
        .parse()
        .map_err(|_| CalibrationError::Metric(value.to_owned()))
}
fn relative_error(exact: u128, estimate: u128) -> i128 {
    if exact == 0 {
        return if estimate == 0 { 0 } else { 1_000_000 };
    }
    let difference = estimate as i128 - exact as i128;
    difference.saturating_mul(1_000_000) / exact as i128
}

#[derive(Debug, Error)]
pub enum CalibrationError {
    #[error("individual engine calibration failed: {0}")]
    Engine(String),
    #[error(transparent)]
    Scale(#[from] crate::ScaleError),
    #[error(transparent)]
    Hybrid(#[from] crate::HybridError),
    #[error("invalid metric value {0}")]
    Metric(String),
}
