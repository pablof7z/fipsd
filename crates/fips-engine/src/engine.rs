//! Current-FIPS Root Ratchet reference execution and artifact projection.

use crate::{
    AttachmentSelector, Delivery, EnqueueRequest, EventId, GraphError, GraphMemoryFootprint,
    GraphStore, LinkClass, LinkConfig, LinkCounters, LinkError, LinkOrdering, LinkService,
    NodeAddress, NodeId, RecoveryEngine, RecoveryReport, ScheduleError, Scheduler,
    SchedulerDiagnostics, TopologyKind,
};
use fips_adapter::{CodecManifest, FIPS_COMMIT};
use fips_artifact::{
    AssertionResult, BloomFidelity, ComputeFidelity, EventRecord, FidelityContract, LedgerEntry,
    MetricPoint, MetricSeries, ProtocolFidelity, ProvenanceEnvelope, REPRODUCTION_BUNDLE_VERSION,
    RUN_ARTIFACT_VERSION, ReproductionBundle, RunArtifact, RunManifest, ScaleFidelity,
    WireFidelity,
};
use fips_engine_api::{Engine, EngineEffect, EngineError, EngineIdentity, EngineRequest};
use fips_model::{ModelError, NORMALIZED_PLAN_VERSION, NormalizedPlan};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const ENGINE_NAME: &str = "fips-individual-reference";
const ENGINE_VERSION: &str = "m2-v1";
const DEFAULT_DEBOUNCE_NS: u64 = 500_000_000;
const MAX_EVENTS: usize = 1_000_000;

/// Reconciled TreeAnnounce lifecycle counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeAnnounceCounters {
    /// State transitions asking for a per-peer announcement.
    pub requested: u64,
    /// Pending requests replaced before construction.
    pub superseded: u64,
    /// Replacements folded into a later construction.
    pub coalesced: u64,
    /// Requests cancelled before construction because an endpoint became inactive.
    pub cancelled: u64,
    /// Announcements constructed from the latest state.
    pub constructed: u64,
    /// Signature operations requested.
    pub signed: u64,
    /// Frames serialized with executable-codec-derived sizes.
    pub serialized: u64,
    /// Frames accepted by a link queue.
    pub queued: u64,
    /// Frames rejected by MTU or queue policy.
    pub rejected: u64,
    /// Wire copies transmitted, including copies later lost.
    pub transmitted: u64,
    /// Wire copies delivered.
    pub delivered: u64,
    /// Exact FMP frame bytes transmitted.
    pub transmitted_frame_bytes: u64,
    /// Exact FMP frame bytes delivered.
    pub delivered_frame_bytes: u64,
}

/// Headless M1 report embedded in every run artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootRatchetReport {
    /// Report type discriminator.
    pub kind: String,
    /// Stable run ID.
    pub run_id: String,
    /// Campaign seed.
    pub seed: u64,
    /// Current-FIPS semantics pin.
    pub upstream_fips_commit: String,
    /// Self-contained fidelity statement for non-UI consumers.
    pub fidelity_statement: String,
    /// Stable graph hash after all arrivals attach.
    pub graph_sha256: String,
    /// Represented individual nodes.
    pub node_count: u64,
    /// Accepted descending-root arrivals.
    pub arrivals: u64,
    /// Identity-generation trials charged to the attacker.
    pub identity_generation_trials: u64,
    /// Final minimum root address.
    pub final_root: String,
    /// Number of distinct adopted root generations.
    pub root_generations: Vec<String>,
    /// Deepest final coordinate path in edges.
    pub maximum_depth: u64,
    /// Accepted parent changes.
    pub parent_transitions: u64,
    /// Root/tree quiescence time.
    pub quiescence_ns: u64,
    /// Tree lifecycle accounting.
    pub tree_announce: TreeAnnounceCounters,
    /// Stable directed-edge counters.
    pub links: BTreeMap<String, LinkCounters>,
    /// Scheduler diagnostics.
    pub scheduler: SchedulerDiagnostics,
    /// Compact graph memory estimate.
    pub graph_memory: GraphMemoryFootprint,
    /// Assertion results, copied from the artifact for standalone inspection.
    pub assertions: Vec<AssertionResult>,
}

/// Complete deterministic individual-engine result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndividualRun {
    /// Immutable run artifact.
    pub artifact: RunArtifact,
    /// Replayable compact bundle.
    pub reproduction: ReproductionBundle,
    /// First explanatory report.
    pub report: RootRatchetReport,
    /// Coupled M2 recovery report when requested by the Campaign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_report: Option<RecoveryReport>,
}

/// M1 exact individual-node engine.
#[derive(Debug, Clone, Default)]
pub struct IndividualEngine;

impl IndividualEngine {
    /// Execute one fully resolved normalized case.
    pub fn run_plan(&self, plan: &NormalizedPlan) -> Result<IndividualRun, RunError> {
        let config = RunConfig::from_plan(plan)?;
        let state = Simulation::new(plan.clone(), config)?.run()?;
        let mut run = state.finish()?;
        if RecoveryEngine::requested(plan) {
            let recovery = RecoveryEngine.run(plan, &run.report)?;
            let (bloom, approximations) = recovery.fidelity();
            run.artifact.manifest.fidelity.bloom = bloom;
            run.artifact.manifest.fidelity.approximations = approximations.clone();
            run.reproduction.fidelity.bloom = bloom;
            run.reproduction.fidelity.approximations = approximations;
            run.artifact.metric_series.extend(recovery.metric_series());
            run.artifact
                .causal_ledger
                .extend(recovery.causal_ledger.clone());
            run.artifact
                .assertion_results
                .extend(recovery.assertions.clone());
            run.reproduction.expected_assertions.extend(
                recovery
                    .assertions
                    .iter()
                    .map(|assertion| assertion.id.clone()),
            );
            run.artifact.samples.push(serde_json::to_value(&recovery)?);
            run.artifact.validate()?;
            run.recovery_report = Some(recovery);
        }
        Ok(run)
    }
}

impl Engine for IndividualEngine {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            name: ENGINE_NAME.to_owned(),
            version: ENGINE_VERSION.to_owned(),
            source_revision: engine_source_revision(),
        }
    }

    fn validate(&self, request: &EngineRequest) -> Result<(), EngineError> {
        if request.variant != "fips-80c956a-baseline" {
            return Err(EngineError::Unsupported(format!(
                "individual M1 engine does not support variant {}",
                request.variant
            )));
        }
        RunConfig::from_plan(&request.plan)
            .map(|_| ())
            .map_err(|error| EngineError::Unsupported(error.to_string()))
    }

    fn run(&self, request: &EngineRequest) -> Result<Vec<EngineEffect>, EngineError> {
        self.validate(request)?;
        let run = self
            .run_plan(&request.plan)
            .map_err(|error| EngineError::Invariant(error.to_string()))?;
        Ok(run
            .artifact
            .event_trace
            .into_iter()
            .map(|event| EngineEffect {
                causal_id: event.event_id,
                ordinal: event.ordinal,
                kind: event.kind,
                payload: event.data,
                virtual_time_ns: event.virtual_time_ns,
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
struct RunConfig {
    nodes: u32,
    arrivals: u32,
    topology: TopologyKind,
    average_degree: u32,
    explicit_edges: Vec<(NodeId, NodeId)>,
    attachment: AttachmentSelector,
    address_policy: String,
    precomputed_ladder: Vec<NodeAddress>,
    attacker_budget_mode: String,
    attacker_operations: Option<u64>,
    arrival_start_ns: u64,
    arrival_interval_ns: u64,
    debounce_ns: u64,
    parent_hysteresis_ppm: u32,
    parent_hold_down_ns: u64,
    link: LinkConfig,
    inject_parent_loop_at_ns: Option<u64>,
    lifecycle: Vec<LifecycleInput>,
}

#[derive(Debug, Clone)]
struct LifecycleInput {
    at_ns: u64,
    node: NodeId,
    reappear: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TreeSnapshot {
    root: NodeId,
    root_address: NodeAddress,
    parent: Option<NodeId>,
    sequence: u64,
    ancestry: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum SimEvent {
    InitialAnnounce,
    Activate {
        node: NodeId,
        ordinal: u32,
    },
    AnnounceDue {
        from: NodeId,
        to: NodeId,
        cause: String,
    },
    DeliverAnnounce {
        delivery: Delivery,
        snapshot: TreeSnapshot,
        cause: String,
    },
    InjectParentLoop,
    Deactivate {
        node: NodeId,
    },
    Reappear {
        node: NodeId,
    },
}

impl SimEvent {
    fn kind(&self) -> &'static str {
        match self {
            Self::InitialAnnounce => "input.initial-topology",
            Self::Activate { .. } => "input.descending-root-arrival",
            Self::AnnounceDue { .. } => "tree-announce.due",
            Self::DeliverAnnounce { .. } => "tree-announce.delivered",
            Self::InjectParentLoop => "fault.inject-parent-loop",
            Self::Deactivate { .. } => "input.node-disappeared",
            Self::Reappear { .. } => "input.node-reappeared",
        }
    }
}

#[derive(Debug, Clone)]
struct PendingAnnounce {
    event_id: EventId,
    cause: String,
}

#[derive(Debug, Clone, Default)]
struct LedgerAccumulator {
    count: u64,
    evidence: Vec<String>,
}

struct Simulation {
    plan: NormalizedPlan,
    config: RunConfig,
    graph: GraphStore,
    scheduler: Scheduler<SimEvent>,
    links: LinkService,
    peer_views: BTreeMap<(NodeId, NodeId), TreeSnapshot>,
    pending: BTreeMap<(NodeId, NodeId), PendingAnnounce>,
    last_sent_ns: BTreeMap<(NodeId, NodeId), u64>,
    sent_times: BTreeMap<(NodeId, NodeId), Vec<u64>>,
    last_parent_switch_ns: Vec<Option<u64>>,
    trace: Vec<EventRecord>,
    ledger: BTreeMap<(String, String), LedgerAccumulator>,
    tree: TreeAnnounceCounters,
    root_generations: BTreeSet<NodeAddress>,
    parent_transitions: u64,
    identity_trials: u64,
    accepted_arrivals: u64,
}

#[path = "engine_config.rs"]
mod config;
#[path = "engine_events.rs"]
mod events;
#[path = "engine_finish.rs"]
mod finish;
#[path = "engine_runtime.rs"]
mod runtime;
#[path = "engine_tree.rs"]
mod tree;

#[derive(Debug, Clone)]
struct Transition {
    parent: Option<NodeId>,
    ancestry: Vec<NodeId>,
    mandatory: bool,
}

/// Individual-engine failure.
#[derive(Debug, Error)]
pub enum RunError {
    /// Unresolved/unsupported input.
    #[error("unsupported individual-engine case: {0}")]
    Unsupported(String),
    /// A scientific invariant failed.
    #[error("individual-engine invariant failed: {0}")]
    Invariant(String),
    /// Identity-generation budget exhausted.
    #[error(
        "attacker identity budget exhausted: required {required} operations, available {available}"
    )]
    BudgetExhausted {
        /// Required cumulative operations.
        required: u64,
        /// Configured budget.
        available: u64,
    },
    /// Arithmetic overflow.
    #[error("individual-engine arithmetic overflow")]
    Arithmetic,
    /// Graph contract failure.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Scheduler contract failure.
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
    /// Link contract failure.
    #[error(transparent)]
    Link(#[from] LinkError),
    /// Normalized-plan serialization failure.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// Artifact contract failure.
    #[error(transparent)]
    Artifact(#[from] fips_artifact::ArtifactError),
    /// JSON conversion failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Codec manifest failure.
    #[error("codec manifest error: {0}")]
    Codec(String),
    /// Coupled M2 recovery failure.
    #[error(transparent)]
    Recovery(#[from] crate::RecoveryError),
}

fn scalar_value<'a>(campaign: &'a Value, path: &str) -> Result<&'a Value, RunError> {
    let value = campaign
        .pointer(path)
        .ok_or_else(|| RunError::Unsupported(format!("missing required value at {path}")))?;
    if value.get("values").is_some() {
        return Err(RunError::Unsupported(format!(
            "unresolved value-set axis at {path}; compile a concrete case first"
        )));
    }
    Ok(value)
}

fn scalar_str<'a>(campaign: &'a Value, path: &str) -> Result<&'a str, RunError> {
    scalar_value(campaign, path)?
        .as_str()
        .ok_or_else(|| RunError::Unsupported(format!("expected string at {path}")))
}

fn scalar_u64(campaign: &Value, path: &str) -> Result<u64, RunError> {
    scalar_value(campaign, path)?
        .as_u64()
        .ok_or_else(|| RunError::Unsupported(format!("expected unsigned integer at {path}")))
}

fn optional_str<'a>(campaign: &'a Value, path: &str) -> Result<Option<&'a str>, RunError> {
    match campaign.pointer(path) {
        None => Ok(None),
        Some(value) if value.get("values").is_some() => Err(RunError::Unsupported(format!(
            "unresolved value-set axis at {path}; compile a concrete case first"
        ))),
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| RunError::Unsupported(format!("expected string at {path}"))),
    }
}

fn optional_u64(campaign: &Value, path: &str) -> Result<Option<u64>, RunError> {
    match campaign.pointer(path) {
        None => Ok(None),
        Some(value) if value.get("values").is_some() => Err(RunError::Unsupported(format!(
            "unresolved value-set axis at {path}; compile a concrete case first"
        ))),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| RunError::Unsupported(format!("expected unsigned integer at {path}"))),
    }
}

fn optional_duration(campaign: &Value, path: &str) -> Result<Option<u64>, RunError> {
    duration_value(campaign.pointer(path))
}

fn duration_value(value: Option<&Value>) -> Result<Option<u64>, RunError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.get("values").is_some() {
        return Err(RunError::Unsupported(
            "unresolved duration value-set axis; compile a concrete case first".to_owned(),
        ));
    }
    value
        .get("nanoseconds")
        .and_then(Value::as_u64)
        .map(Some)
        .ok_or_else(|| RunError::Unsupported("expected normalized duration".to_owned()))
}

fn metric(name: &str, unit: &str, at: u64, value: u64) -> MetricSeries {
    MetricSeries {
        name: name.to_owned(),
        unit: unit.to_owned(),
        points: vec![MetricPoint {
            virtual_time_ns: at,
            value: value.to_string(),
        }],
    }
}

fn engine_source_revision() -> String {
    let mut hasher = Sha256::new();
    hasher.update(include_bytes!("engine.rs"));
    hasher.update(include_bytes!("engine_config.rs"));
    hasher.update(include_bytes!("engine_events.rs"));
    hasher.update(include_bytes!("engine_finish.rs"));
    hasher.update(include_bytes!("engine_runtime.rs"));
    hasher.update(include_bytes!("engine_tree.rs"));
    hasher.update(include_bytes!("graph.rs"));
    hasher.update(include_bytes!("graph_generators.rs"));
    hasher.update(include_bytes!("network.rs"));
    hasher.update(include_bytes!("bloom.rs"));
    hasher.update(include_bytes!("cache.rs"));
    hasher.update(include_bytes!("lookup.rs"));
    hasher.update(include_bytes!("recovery.rs"));
    hasher.update(include_bytes!("recovery_config.rs"));
    hasher.update(include_bytes!("recovery_finish.rs"));
    hasher.update(include_bytes!("recovery_flow.rs"));
    hasher.update(include_bytes!("recovery_helpers.rs"));
    hasher.update(include_bytes!("recovery_io.rs"));
    hasher.update(include_bytes!("recovery_runtime.rs"));
    hasher.update(include_bytes!("recovery_state.rs"));
    hasher.update(include_bytes!("resources.rs"));
    hasher.update(include_bytes!("scheduler.rs"));
    hasher.update(include_bytes!("traffic.rs"));
    hasher.update(include_bytes!("../Cargo.toml"));
    hex::encode(hasher.finalize())[..40].to_owned()
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
