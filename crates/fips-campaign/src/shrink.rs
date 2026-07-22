//! Hierarchical deterministic shrinking with cached, parallel predicate trials.

use crate::GeneratedInput;
use fips_artifact::ReproductionBundle;
use fips_engine::IndividualEngine;
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

/// Ordered hierarchical shrink dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShrinkDimension {
    Traffic,
    TopologyRegions,
    Nodes,
    Edges,
    RootTransitions,
    EventTiming,
    ProtocolParameters,
    ResourceClasses,
    Transports,
}

/// One auditable predicate trial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShrinkStep {
    pub ordinal: u64,
    pub dimension: ShrinkDimension,
    pub before_sha256: String,
    pub candidate_sha256: String,
    pub change: String,
    pub predicate_held: bool,
    pub cached: bool,
}

/// Standalone shrink result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShrinkResult {
    pub kind: String,
    pub source_case_id: String,
    pub predicate: String,
    pub initial_plan_sha256: String,
    pub final_plan: NormalizedPlan,
    pub steps: Vec<ShrinkStep>,
    pub cache_entries: usize,
    pub reproduction: Option<ReproductionBundle>,
}

/// Predicate that defines the failure or objective threshold to preserve.
pub trait ShrinkPredicate: Send + Sync {
    fn name(&self) -> &str;
    fn holds(&self, plan: &NormalizedPlan) -> bool;
}

impl<F> ShrinkPredicate for (&'static str, F)
where
    F: Fn(&NormalizedPlan) -> bool + Send + Sync,
{
    fn name(&self) -> &str {
        self.0
    }

    fn holds(&self, plan: &NormalizedPlan) -> bool {
        (self.1)(plan)
    }
}

/// Hierarchical plan shrinker.
#[derive(Debug, Clone)]
pub struct HierarchicalShrinker {
    pub worker_count: usize,
}

impl Default for HierarchicalShrinker {
    fn default() -> Self {
        Self { worker_count: 1 }
    }
}

impl HierarchicalShrinker {
    /// Shrink across every M3 dimension while preserving the predicate.
    pub fn shrink<P: ShrinkPredicate + 'static>(
        &self,
        source_case_id: impl Into<String>,
        plan: NormalizedPlan,
        predicate: P,
    ) -> Result<ShrinkResult, ShrinkError> {
        if !predicate.holds(&plan) {
            return Err(ShrinkError::InitialPredicate);
        }
        let predicate = Arc::new(predicate);
        let initial_plan_sha256 = plan.campaign_sha256.clone();
        let mut current = plan;
        let mut steps = Vec::new();
        let mut cache = BTreeMap::from([(current.campaign_sha256.clone(), true)]);
        let dimensions = [
            ShrinkDimension::Traffic,
            ShrinkDimension::TopologyRegions,
            ShrinkDimension::Nodes,
            ShrinkDimension::Edges,
            ShrinkDimension::RootTransitions,
            ShrinkDimension::EventTiming,
            ShrinkDimension::ProtocolParameters,
            ShrinkDimension::ResourceClasses,
            ShrinkDimension::Transports,
        ];
        let mut ordinal = 0_u64;
        for dimension in dimensions {
            loop {
                let candidates = candidates(&current, dimension)?;
                if candidates.is_empty() {
                    break;
                }
                let mut trials = Vec::new();
                let mut pending = Vec::new();
                for (candidate, change) in candidates {
                    if let Some(held) = cache.get(&candidate.campaign_sha256).copied() {
                        trials.push((candidate, change, held, true));
                    } else {
                        pending.push((candidate, change));
                    }
                }
                let evaluated =
                    evaluate_parallel(pending, Arc::clone(&predicate), self.worker_count.max(1));
                for (candidate, change, held) in evaluated {
                    cache.insert(candidate.campaign_sha256.clone(), held);
                    trials.push((candidate, change, held, false));
                }
                trials.sort_by(|left, right| left.0.campaign_sha256.cmp(&right.0.campaign_sha256));
                let before = current.campaign_sha256.clone();
                let accepted = trials
                    .iter()
                    .find(|(_, _, held, _)| *held)
                    .map(|(candidate, _, _, _)| candidate.clone());
                for (candidate, change, held, cached) in trials {
                    steps.push(ShrinkStep {
                        ordinal,
                        dimension,
                        before_sha256: before.clone(),
                        candidate_sha256: candidate.campaign_sha256.clone(),
                        change,
                        predicate_held: held,
                        cached,
                    });
                    ordinal += 1;
                }
                let Some(next) = accepted else { break };
                if next.campaign_sha256 == current.campaign_sha256 {
                    break;
                }
                current = next;
            }
        }
        let reproduction = IndividualEngine
            .run_plan(&current)
            .ok()
            .map(|run| run.reproduction);
        Ok(ShrinkResult {
            kind: "hierarchical-shrink/v1alpha1".to_owned(),
            source_case_id: source_case_id.into(),
            predicate: predicate.name().to_owned(),
            initial_plan_sha256,
            final_plan: current,
            steps,
            cache_entries: cache.len(),
            reproduction,
        })
    }
}

/// Shrink a property-generated input before or alongside trace shrinking.
pub fn shrink_generated<P>(mut input: GeneratedInput, predicate: P) -> GeneratedInput
where
    P: Fn(&GeneratedInput) -> bool,
{
    loop {
        let current_complexity = generated_complexity(&input);
        let next = input
            .shrink_candidates()
            .into_iter()
            .filter(|candidate| {
                generated_complexity(candidate) < current_complexity && predicate(candidate)
            })
            .min_by_key(generated_complexity);
        match next {
            Some(next) => input = next,
            None => return input,
        }
    }
}

fn generated_complexity(input: &GeneratedInput) -> (u64, usize, u64) {
    (
        input.topology.represented_nodes,
        input.events.len(),
        input.constraints.minimum_interval_ns,
    )
}

/// Shrink failure.
#[derive(Debug, Error)]
pub enum ShrinkError {
    #[error("initial case does not satisfy the selected shrink predicate")]
    InitialPredicate,
    #[error("cannot serialize a shrink candidate: {0}")]
    Json(#[from] serde_json::Error),
}

fn candidates(
    plan: &NormalizedPlan,
    dimension: ShrinkDimension,
) -> Result<Vec<(NormalizedPlan, String)>, ShrinkError> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    let mutations = mutations(&plan.campaign, dimension);
    for (campaign, change) in mutations {
        let candidate = resolved_plan(plan.seed, campaign)?;
        if candidate.campaign_sha256 != plan.campaign_sha256
            && seen.insert(candidate.campaign_sha256.clone())
        {
            output.push((candidate, change));
        }
    }
    Ok(output)
}

fn mutations(campaign: &Value, dimension: ShrinkDimension) -> Vec<(Value, String)> {
    let mut output = Vec::new();
    match dimension {
        ShrinkDimension::Traffic => {
            halve(campaign, "/traffic/parameters/flow_count", 1, &mut output);
            halve(campaign, "/traffic/payload_bytes", 1, &mut output);
        }
        ShrinkDimension::TopologyRegions => {
            set(
                campaign,
                "/topology/generator",
                Value::String("chain".to_owned()),
                &mut output,
            );
        }
        ShrinkDimension::Nodes => halve(campaign, "/scale/nodes", 2, &mut output),
        ShrinkDimension::Edges => halve(campaign, "/topology/average_degree", 1, &mut output),
        ShrinkDimension::RootTransitions => {
            halve(campaign, "/identities/arrivals/count", 1, &mut output)
        }
        ShrinkDimension::EventTiming => halve_duration(
            campaign,
            "/identities/arrivals/schedule/interval",
            &mut output,
        ),
        ShrinkDimension::ProtocolParameters => {
            remove_last_parameter(campaign, "/protocol/parameters", &mut output)
        }
        ShrinkDimension::ResourceClasses => {
            set(
                campaign,
                "/resources/assignment",
                Value::String("uniform".to_owned()),
                &mut output,
            );
            truncate_array(campaign, "/resources/node_profiles", &mut output);
        }
        ShrinkDimension::Transports => set(
            campaign,
            "/transports/assignment",
            Value::String("all-udp".to_owned()),
            &mut output,
        ),
    }
    output
}

fn halve(campaign: &Value, pointer: &str, minimum: u64, output: &mut Vec<(Value, String)>) {
    let Some(value) = campaign.pointer(pointer).and_then(Value::as_u64) else {
        return;
    };
    let next = value.div_ceil(2).max(minimum);
    if next != value {
        set(campaign, pointer, Value::from(next), output);
    }
}

fn halve_duration(campaign: &Value, pointer: &str, output: &mut Vec<(Value, String)>) {
    let Some(value) = campaign
        .pointer(pointer)
        .and_then(|value| value.get("nanoseconds"))
        .and_then(Value::as_u64)
    else {
        return;
    };
    set(
        campaign,
        pointer,
        serde_json::json!({"nanoseconds": value.div_ceil(2)}),
        output,
    );
}

fn set(campaign: &Value, pointer: &str, value: Value, output: &mut Vec<(Value, String)>) {
    if campaign.pointer(pointer) == Some(&value) {
        return;
    }
    let mut candidate = campaign.clone();
    if let Some(slot) = candidate.pointer_mut(pointer) {
        let before = slot.clone();
        *slot = value.clone();
        output.push((candidate, format!("{pointer}: {before} -> {value}")));
    }
}

fn truncate_array(campaign: &Value, pointer: &str, output: &mut Vec<(Value, String)>) {
    let Some(values) = campaign.pointer(pointer).and_then(Value::as_array) else {
        return;
    };
    if values.len() > 1 {
        let mut candidate = campaign.clone();
        candidate
            .pointer_mut(pointer)
            .and_then(Value::as_array_mut)
            .expect("pointer was an array")
            .truncate(1);
        output.push((candidate, format!("{pointer}: retained first profile")));
    }
}

fn remove_last_parameter(campaign: &Value, pointer: &str, output: &mut Vec<(Value, String)>) {
    let Some(object) = campaign.pointer(pointer).and_then(Value::as_object) else {
        return;
    };
    let Some(key) = object.keys().next_back() else {
        return;
    };
    let mut candidate = campaign.clone();
    candidate
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .expect("pointer was an object")
        .remove(key);
    output.push((candidate, format!("{pointer}: removed {key}")));
}

fn resolved_plan(seed: u64, campaign: Value) -> Result<NormalizedPlan, serde_json::Error> {
    let bytes = serde_json::to_vec(&campaign)?;
    Ok(NormalizedPlan {
        api_version: fips_model::NORMALIZED_PLAN_VERSION.to_owned(),
        campaign_sha256: hex::encode(Sha256::digest(bytes)),
        campaign,
        axes: Vec::new(),
        seed,
    })
}

fn evaluate_parallel<P: ShrinkPredicate + 'static>(
    candidates: Vec<(NormalizedPlan, String)>,
    predicate: Arc<P>,
    workers: usize,
) -> Vec<(NormalizedPlan, String, bool)> {
    if workers == 1 || candidates.len() < 2 {
        return candidates
            .into_iter()
            .map(|(plan, change)| {
                let held = predicate.holds(&plan);
                (plan, change, held)
            })
            .collect();
    }
    let mut output = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in candidates.chunks(candidates.len().div_ceil(workers)) {
            let predicate = Arc::clone(&predicate);
            let chunk = chunk.to_vec();
            handles.push(scope.spawn(move || {
                chunk
                    .into_iter()
                    .map(|(plan, change)| {
                        let held = predicate.holds(&plan);
                        (plan, change, held)
                    })
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            output.extend(handle.join().expect("shrink predicate worker panicked"));
        }
    });
    output
}
