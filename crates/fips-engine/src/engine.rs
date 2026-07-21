//! Current-FIPS Root Ratchet reference execution and artifact projection.

use crate::{
    AttachmentSelector, Delivery, EnqueueRequest, EventId, GraphError, GraphMemoryFootprint,
    GraphStore, LinkClass, LinkConfig, LinkCounters, LinkError, LinkOrdering, LinkService,
    NodeAddress, NodeId, ScheduleError, Scheduler, SchedulerDiagnostics, TopologyKind,
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
const ENGINE_VERSION: &str = "m1-v1";
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
}

/// M1 exact individual-node engine.
#[derive(Debug, Clone, Default)]
pub struct IndividualEngine;

impl IndividualEngine {
    /// Execute one fully resolved normalized case.
    pub fn run_plan(&self, plan: &NormalizedPlan) -> Result<IndividualRun, RunError> {
        let config = RunConfig::from_plan(plan)?;
        let state = Simulation::new(plan.clone(), config)?.run()?;
        state.finish()
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

impl RunConfig {
    fn from_plan(plan: &NormalizedPlan) -> Result<Self, RunError> {
        let campaign = &plan.campaign;
        let mode = scalar_str(campaign, "/engine/modes")?;
        if mode != "compact-discrete-event" {
            return Err(RunError::Unsupported(format!(
                "/engine/modes must be compact-discrete-event, got {mode}"
            )));
        }
        let nodes = scalar_u64(campaign, "/scale/nodes")?;
        if !(2..=2_000_000).contains(&nodes) {
            return Err(RunError::Unsupported(format!(
                "individual engine node count {nodes} is outside 2..=2000000"
            )));
        }
        let arrivals = optional_u64(campaign, "/identities/arrivals/count")?.unwrap_or(0);
        if arrivals >= nodes {
            return Err(RunError::Unsupported(format!(
                "arrival count {arrivals} must be less than node count {nodes}"
            )));
        }
        let topology = TopologyKind::parse(scalar_str(campaign, "/topology/generator")?)?;
        let average_degree = optional_u64(campaign, "/topology/average_degree")?.unwrap_or(2);
        let explicit_edges = campaign
            .pointer("/topology/explicit_edges")
            .and_then(Value::as_array)
            .map(|edges| {
                edges
                    .iter()
                    .map(|edge| {
                        let pair = edge.as_array().ok_or_else(|| {
                            RunError::Unsupported("explicit edge must be an array".to_owned())
                        })?;
                        Ok((
                            pair[0].as_u64().unwrap_or(u64::MAX) as NodeId,
                            pair[1].as_u64().unwrap_or(u64::MAX) as NodeId,
                        ))
                    })
                    .collect::<Result<Vec<_>, RunError>>()
            })
            .transpose()?
            .unwrap_or_default();
        let attachment = optional_str(campaign, "/identities/arrivals/attachment")?
            .map(AttachmentSelector::parse)
            .transpose()?
            .unwrap_or(AttachmentSelector::CurrentRoot);
        let address_policy = optional_str(campaign, "/identities/arrivals/address_policy")?
            .unwrap_or("strictly-lower-than-current-root")
            .to_owned();
        if !matches!(
            address_policy.as_str(),
            "one-lower"
                | "one-lower-than-current-root"
                | "strictly-descending"
                | "strictly-lower-than-current-root"
                | "precomputed-ladder"
        ) {
            return Err(RunError::Unsupported(format!(
                "unsupported identity address policy {address_policy}"
            )));
        }
        let precomputed_ladder = campaign
            .pointer("/identities/arrivals/precomputed_ladder")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or_else(|| {
                                RunError::Unsupported(
                                    "precomputed ladder addresses must be strings".to_owned(),
                                )
                            })
                            .and_then(|value| NodeAddress::from_hex(value).map_err(Into::into))
                    })
                    .collect::<Result<Vec<_>, RunError>>()
            })
            .transpose()?
            .unwrap_or_default();
        if address_policy == "precomputed-ladder" && precomputed_ladder.len() < arrivals as usize {
            return Err(RunError::Unsupported(format!(
                "precomputed ladder has {} addresses for {arrivals} arrivals",
                precomputed_ladder.len()
            )));
        }
        let attacker_budget_mode =
            optional_str(campaign, "/identities/arrivals/attacker_budget/mode")?
                .unwrap_or("free-input")
                .to_owned();
        let attacker_operations =
            optional_u64(campaign, "/identities/arrivals/attacker_budget/operations")?;
        let arrival_start_ns = optional_duration(campaign, "/identities/arrivals/schedule/start")?
            .unwrap_or(2_000_000_000);
        let arrival_interval_ns =
            optional_duration(campaign, "/identities/arrivals/schedule/interval")?
                .unwrap_or(500_000_000);
        if arrival_interval_ns > 10_000_000_000 {
            return Err(RunError::Unsupported(
                "arrival cadence must be between simultaneous and 10s".to_owned(),
            ));
        }
        let debounce_ns =
            optional_duration(campaign, "/protocol/parameters/tree_announce_debounce")?
                .unwrap_or(DEFAULT_DEBOUNCE_NS);
        let parent_hysteresis_ppm =
            optional_u64(campaign, "/protocol/parameters/parent_hysteresis_ppm")?.unwrap_or(0)
                as u32;
        let parent_hold_down_ns =
            optional_duration(campaign, "/protocol/parameters/parent_hold_down")?.unwrap_or(0);
        let ordering = match optional_str(campaign, "/links/ordering")?.unwrap_or("datagram") {
            "datagram" => LinkOrdering::Datagram,
            "stream" => LinkOrdering::Stream,
            other => {
                return Err(RunError::Unsupported(format!(
                    "unsupported link ordering {other}"
                )));
            }
        };
        let transport = scalar_str(campaign, "/transports/assignment")?;
        let transport_overhead_bytes = match transport {
            "all-udp" => 28,
            "all-tcp" => 40,
            "all-ethernet" => 18,
            other => {
                return Err(RunError::Unsupported(format!(
                    "M1 individual engine requires a homogeneous transport, got {other}"
                )));
            }
        };
        let link = LinkConfig {
            latency_ns: optional_duration(campaign, "/links/latency")?.unwrap_or(1_000_000),
            bandwidth_bps: optional_u64(campaign, "/links/bandwidth_bps")?.unwrap_or(1_000_000_000),
            loss_ppm: optional_u64(campaign, "/links/loss_ppm")?.unwrap_or(0) as u32,
            duplication_ppm: optional_u64(campaign, "/links/duplication_ppm")?.unwrap_or(0) as u32,
            ordering,
            mtu_bytes: optional_u64(campaign, "/links/mtu_bytes")?.unwrap_or(9_000),
            queue_bytes: optional_u64(campaign, "/links/queue_bytes")?.unwrap_or(1_048_576),
            transport_overhead_bytes,
        };
        let inject_parent_loop_event = campaign
            .pointer("/events")
            .and_then(Value::as_array)
            .and_then(|events| {
                events.iter().find(|event| {
                    event.get("action").and_then(Value::as_str) == Some("inject-parent-loop")
                })
            });
        let inject_parent_loop_at_ns = match inject_parent_loop_event {
            Some(event) => duration_value(event.get("at"))?,
            None => None,
        };
        let lifecycle = campaign
            .pointer("/events")
            .and_then(Value::as_array)
            .map(|events| {
                events
                    .iter()
                    .filter_map(|event| {
                        let action = event.get("action").and_then(Value::as_str)?;
                        let reappear = match action {
                            "disappear-node" => false,
                            "reappear-node" => true,
                            _ => return None,
                        };
                        Some((event, reappear))
                    })
                    .map(|(event, reappear)| {
                        let at_ns = duration_value(event.get("at"))?.ok_or_else(|| {
                            RunError::Unsupported(
                                "disappear/reappear event requires an at duration".to_owned(),
                            )
                        })?;
                        let node = event
                            .get("target")
                            .and_then(Value::as_u64)
                            .or_else(|| event.pointer("/parameters/node").and_then(Value::as_u64))
                            .ok_or_else(|| {
                                RunError::Unsupported(
                                    "disappear/reappear event requires an integer target"
                                        .to_owned(),
                                )
                            })?;
                        if node >= nodes {
                            return Err(RunError::Unsupported(format!(
                                "lifecycle event targets node {node}, but scale is {nodes}"
                            )));
                        }
                        Ok(LifecycleInput {
                            at_ns,
                            node: node as NodeId,
                            reappear,
                        })
                    })
                    .collect::<Result<Vec<_>, RunError>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            nodes: nodes as u32,
            arrivals: arrivals as u32,
            topology,
            average_degree: average_degree as u32,
            explicit_edges,
            attachment,
            address_policy,
            precomputed_ladder,
            attacker_budget_mode,
            attacker_operations,
            arrival_start_ns,
            arrival_interval_ns,
            debounce_ns,
            parent_hysteresis_ppm,
            parent_hold_down_ns,
            link,
            inject_parent_loop_at_ns,
            lifecycle,
        })
    }
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

impl Simulation {
    fn new(plan: NormalizedPlan, config: RunConfig) -> Result<Self, RunError> {
        let mut graph = GraphStore::generate(
            config.topology,
            config.nodes,
            config.average_degree,
            plan.seed,
            &config.explicit_edges,
        )?;
        graph.reserve_arrivals(config.arrivals)?;
        let initial_root = graph
            .node_ids()
            .filter(|id| graph.is_active(*id))
            .min_by_key(|id| graph.address(*id).ok())
            .ok_or_else(|| RunError::Invariant("no initial root".to_owned()))?;
        let root_address = graph.address(initial_root)?;
        let mut root_generations = BTreeSet::new();
        root_generations.insert(root_address);
        let edge_count = graph.edge_count();
        let node_count = graph.node_count();
        let seed = plan.seed;
        Ok(Self {
            plan,
            config: config.clone(),
            graph,
            scheduler: Scheduler::new(MAX_EVENTS),
            links: LinkService::uniform(seed, edge_count, config.link),
            peer_views: BTreeMap::new(),
            pending: BTreeMap::new(),
            last_sent_ns: BTreeMap::new(),
            sent_times: BTreeMap::new(),
            last_parent_switch_ns: vec![None; node_count],
            trace: Vec::new(),
            ledger: BTreeMap::new(),
            tree: TreeAnnounceCounters::default(),
            root_generations,
            parent_transitions: 0,
            identity_trials: 0,
            accepted_arrivals: 0,
        })
    }

    fn run(mut self) -> Result<Self, RunError> {
        self.scheduler
            .schedule_at(0, None, SimEvent::InitialAnnounce)?;
        let first_arrival = self.graph.node_count() as u32 - self.config.arrivals;
        for ordinal in 0..self.config.arrivals {
            let at = self
                .config
                .arrival_interval_ns
                .checked_mul(u64::from(ordinal))
                .and_then(|offset| self.config.arrival_start_ns.checked_add(offset))
                .ok_or(RunError::Arithmetic)?;
            self.scheduler.schedule_at(
                at,
                None,
                SimEvent::Activate {
                    node: first_arrival + ordinal,
                    ordinal,
                },
            )?;
        }
        if let Some(at) = self.config.inject_parent_loop_at_ns {
            self.scheduler
                .schedule_at(at, None, SimEvent::InjectParentLoop)?;
        }
        for lifecycle in &self.config.lifecycle {
            let payload = if lifecycle.reappear {
                SimEvent::Reappear {
                    node: lifecycle.node,
                }
            } else {
                SimEvent::Deactivate {
                    node: lifecycle.node,
                }
            };
            self.scheduler.schedule_at(lifecycle.at_ns, None, payload)?;
        }

        while let Some(event) = self.scheduler.pop() {
            if self.trace.len() >= MAX_EVENTS {
                return Err(RunError::Invariant(format!(
                    "event limit {MAX_EVENTS} exceeded"
                )));
            }
            let event_id = format!("event-{:016x}", event.id);
            let parent = event.parent.map(|id| format!("event-{id:016x}"));
            let event_kind = event.payload.kind().to_owned();
            let data = match event.payload {
                SimEvent::InitialAnnounce => self.handle_initial(event.id, &event_id)?,
                SimEvent::Activate { node, ordinal } => {
                    self.handle_activate(event.id, &event_id, node, ordinal)?
                }
                SimEvent::AnnounceDue { from, to, cause } => {
                    self.handle_announce_due(event.id, &event_id, from, to, &cause)?
                }
                SimEvent::DeliverAnnounce {
                    delivery,
                    snapshot,
                    cause,
                } => self.handle_delivery(event.id, &event_id, &delivery, snapshot, &cause)?,
                SimEvent::InjectParentLoop => {
                    return Err(RunError::Invariant(
                        "loop-freedom: injected parent ancestry contains the receiving node"
                            .to_owned(),
                    ));
                }
                SimEvent::Deactivate { node } => {
                    self.handle_deactivate(event.id, &event_id, node)?
                }
                SimEvent::Reappear { node } => self.handle_reappear(event.id, &event_id, node)?,
            };
            self.trace.push(EventRecord {
                event_id,
                virtual_time_ns: event.virtual_time_ns,
                ordinal: event.ordinal,
                kind: event_kind,
                causal_parent: parent,
                data,
            });
        }
        Ok(self)
    }

    fn handle_initial(&mut self, event: EventId, evidence: &str) -> Result<Value, RunError> {
        let cause = "input:initial-topology";
        self.add_ledger(cause, "performed", 1, evidence);
        let active = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .collect::<Vec<_>>();
        for node in &active {
            self.request_all(*node, cause, Some(event))?;
        }
        Ok(json!({"active_nodes": active.len()}))
    }

    fn handle_activate(
        &mut self,
        event: EventId,
        evidence: &str,
        node: NodeId,
        ordinal: u32,
    ) -> Result<Value, RunError> {
        let cause = format!("input:arrival-{ordinal:04}");
        self.add_ledger(&cause, "requested", 1, evidence);
        let minimum = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .map(|id| self.graph.address(id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min()
            .ok_or_else(|| RunError::Invariant("arrival has no visible root".to_owned()))?;
        let (address, trials) = if self.config.address_policy == "precomputed-ladder" {
            (self.config.precomputed_ladder[ordinal as usize], 0_u64)
        } else {
            (
                minimum.one_lower().ok_or_else(|| {
                    RunError::Invariant("cannot generate an address below the zero root".to_owned())
                })?,
                1_u64,
            )
        };
        if address >= minimum {
            return Err(RunError::Invariant(format!(
                "arrival {ordinal} address {} is not lower than visible root {}",
                address.to_hex(),
                minimum.to_hex()
            )));
        }
        if self.config.attacker_budget_mode == "bounded"
            && self
                .config
                .attacker_operations
                .is_none_or(|budget| self.identity_trials + trials > budget)
        {
            return Err(RunError::BudgetExhausted {
                required: self.identity_trials + trials,
                available: self.config.attacker_operations.unwrap_or(0),
            });
        }
        self.identity_trials += trials;
        self.graph.set_address(node, address)?;
        self.graph.reset_self_root(node)?;
        let target = self.graph.select_attachment(
            self.config.attachment,
            self.plan.seed,
            u64::from(ordinal),
        )?;
        self.graph.set_active(node, true)?;
        let edge = if let Some(edge) = self.graph.edge_between(node, target) {
            edge
        } else {
            let edge = self.graph.add_edge(node, target)?;
            self.links.set_config(edge, self.config.link.clone())?;
            edge
        };
        self.accepted_arrivals += 1;
        self.root_generations.insert(address);
        self.add_ledger(&cause, "performed", 1, evidence);
        self.add_ledger(&cause, "identity-generation-operations", trials, evidence);
        self.request_all(node, &cause, Some(event))?;
        self.request_announce(target, node, &cause, Some(event))?;
        Ok(json!({
            "node": node,
            "address": address.to_hex(),
            "address_policy": self.config.address_policy,
            "attachment": format!("{:?}", self.config.attachment),
            "target": target,
            "edge": edge,
            "identity_trials": trials
        }))
    }

    fn handle_announce_due(
        &mut self,
        event: EventId,
        evidence: &str,
        from: NodeId,
        to: NodeId,
        cause: &str,
    ) -> Result<Value, RunError> {
        if self.pending.get(&(from, to)).map(|item| item.event_id) != Some(event) {
            return Err(RunError::Invariant(format!(
                "orphaned announce event {event} for {from}->{to}"
            )));
        }
        let pending_cause = self.pending.remove(&(from, to)).map(|item| item.cause);
        if pending_cause.as_deref() != Some(cause) {
            return Err(RunError::Invariant(format!(
                "announce cause drift for {from}->{to}"
            )));
        }
        if !self.graph.is_active(from) || !self.graph.is_active(to) {
            self.tree.cancelled += 1;
            self.add_ledger(cause, "cancelled", 1, evidence);
            return Ok(json!({"from": from, "to": to, "skipped": "inactive"}));
        }
        let snapshot = self.snapshot(from)?;
        let depth = snapshot.ancestry.len().saturating_sub(1) as u32;
        let manifest = CodecManifest::load().map_err(|error| RunError::Codec(error.to_string()))?;
        if depth > manifest.maximum_safe_tree_depth {
            return Err(RunError::Unsupported(format!(
                "TreeAnnounce depth {depth} exceeds FMP maximum {}",
                manifest.maximum_safe_tree_depth
            )));
        }
        let frame_bytes = 168_u64 + 32 * u64::from(depth);
        self.tree.constructed += 1;
        self.tree.signed += 1;
        self.tree.serialized += 1;
        self.add_ledger(cause, "constructed", 1, evidence);
        self.add_ledger(cause, "signed", 1, evidence);
        self.add_ledger(cause, "serialized", frame_bytes, evidence);
        let edge = self
            .graph
            .edge_between(from, to)
            .ok_or_else(|| RunError::Invariant(format!("no edge for announce {from}->{to}")))?;
        match self.links.enqueue(EnqueueRequest {
            edge_id: edge,
            from,
            to,
            class: LinkClass::Control,
            frame_bytes,
            useful_payload_bytes: 0,
            now_ns: self.scheduler.now_ns(),
        }) {
            Ok(result) => {
                self.tree.queued += 1;
                self.tree.transmitted += result.transmitted_bytes
                    / (frame_bytes + self.config.link.transport_overhead_bytes);
                self.tree.transmitted_frame_bytes += result.transmitted_bytes.saturating_sub(
                    self.config.link.transport_overhead_bytes
                        * (result.transmitted_bytes
                            / (frame_bytes + self.config.link.transport_overhead_bytes)),
                );
                self.add_ledger(cause, "queued", frame_bytes, evidence);
                self.add_ledger(cause, "transmitted", result.transmitted_bytes, evidence);
                for delivery in result.deliveries {
                    self.scheduler.schedule_at(
                        delivery.deliver_at_ns,
                        Some(event),
                        SimEvent::DeliverAnnounce {
                            delivery,
                            snapshot: snapshot.clone(),
                            cause: cause.to_owned(),
                        },
                    )?;
                }
                self.last_sent_ns
                    .insert((from, to), self.scheduler.now_ns());
                self.sent_times
                    .entry((from, to))
                    .or_default()
                    .push(self.scheduler.now_ns());
                Ok(json!({
                    "from": from,
                    "to": to,
                    "depth": depth,
                    "frame_bytes": frame_bytes,
                    "transport_bytes": result.transmitted_bytes,
                    "lost_bytes": result.lost_bytes,
                    "queue_occupancy_bytes": result.queue_occupancy_bytes
                }))
            }
            Err(error @ (LinkError::MtuExceeded { .. } | LinkError::QueueFull { .. })) => {
                self.tree.rejected += 1;
                self.add_ledger(cause, "rejected", frame_bytes, evidence);
                Ok(json!({
                    "from": from,
                    "to": to,
                    "depth": depth,
                    "frame_bytes": frame_bytes,
                    "rejected": error.to_string()
                }))
            }
            Err(error) => Err(error.into()),
        }
    }

    fn handle_deactivate(
        &mut self,
        event: EventId,
        evidence: &str,
        node: NodeId,
    ) -> Result<Value, RunError> {
        if !self.graph.is_active(node) {
            return Err(RunError::Invariant(format!(
                "cannot disappear inactive node {node}"
            )));
        }
        let cause = format!("input:disappear-{node}");
        let neighbors = self.graph.active_neighbors(node);
        self.graph.set_active(node, false)?;
        self.peer_views
            .retain(|(receiver, sender), _| *receiver != node && *sender != node);
        for neighbor in neighbors {
            if self.graph.parent(neighbor) == Some(node) {
                if let Some(transition) = self.evaluate_parent(neighbor)? {
                    self.apply_transition(neighbor, transition, &cause, Some(event), evidence)?;
                } else {
                    self.graph.reset_self_root(neighbor)?;
                    self.root_generations.insert(self.graph.address(neighbor)?);
                    self.request_all(neighbor, &cause, Some(event))?;
                }
            }
        }
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({"node": node, "active": false}))
    }

    fn handle_reappear(
        &mut self,
        event: EventId,
        evidence: &str,
        node: NodeId,
    ) -> Result<Value, RunError> {
        if self.graph.is_active(node) {
            return Err(RunError::Invariant(format!(
                "cannot reappear active node {node}"
            )));
        }
        let cause = format!("input:reappear-{node}");
        self.graph.set_active(node, true)?;
        self.graph.reset_self_root(node)?;
        self.request_all(node, &cause, Some(event))?;
        for neighbor in self.graph.active_neighbors(node) {
            self.request_announce(neighbor, node, &cause, Some(event))?;
        }
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({"node": node, "active": true}))
    }

    fn handle_delivery(
        &mut self,
        event: EventId,
        evidence: &str,
        delivery: &Delivery,
        snapshot: TreeSnapshot,
        cause: &str,
    ) -> Result<Value, RunError> {
        self.links.record_delivery(delivery, 0)?;
        self.tree.delivered += 1;
        self.tree.delivered_frame_bytes += delivery.frame_bytes;
        self.add_ledger(cause, "delivered", delivery.wire_bytes, evidence);
        let receiver = delivery.to;
        if !self.graph.is_active(receiver) {
            return Ok(json!({
                "edge": delivery.edge_id,
                "from": delivery.from,
                "to": receiver,
                "frame_bytes": delivery.frame_bytes,
                "skipped": "receiver-inactive"
            }));
        }
        if !self.snapshot_semantics_valid(&snapshot)? {
            return Err(RunError::Invariant(format!(
                "accepted-ancestry: invalid snapshot from {}",
                delivery.from
            )));
        }
        let key = (receiver, delivery.from);
        let fresh = self
            .peer_views
            .get(&key)
            .is_none_or(|old| snapshot.sequence > old.sequence || snapshot != *old);
        if fresh {
            self.peer_views.insert(key, snapshot);
        }
        let transition = fresh
            .then(|| self.evaluate_parent(receiver))
            .transpose()?
            .flatten();
        if let Some(transition) = transition {
            self.apply_transition(receiver, transition, cause, Some(event), evidence)?;
        }
        Ok(json!({
            "edge": delivery.edge_id,
            "from": delivery.from,
            "to": delivery.to,
            "frame_bytes": delivery.frame_bytes,
            "copy": delivery.copy_ordinal,
            "fresh": fresh,
            "root": self.graph.address(self.graph.root(receiver))?.to_hex(),
            "parent": self.graph.parent(receiver)
        }))
    }

    fn request_all(
        &mut self,
        from: NodeId,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        let neighbors = self.graph.active_neighbors(from);
        for to in neighbors {
            self.request_announce(from, to, cause, parent)?;
        }
        Ok(())
    }

    fn request_announce(
        &mut self,
        from: NodeId,
        to: NodeId,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        self.tree.requested += 1;
        self.add_ledger(cause, "requested", 1, "");
        let due = self
            .last_sent_ns
            .get(&(from, to))
            .map_or(self.scheduler.now_ns(), |last| {
                self.scheduler
                    .now_ns()
                    .max(last.saturating_add(self.config.debounce_ns))
            });
        if let Some(previous_cause) = self
            .pending
            .get(&(from, to))
            .map(|previous| previous.cause.clone())
        {
            self.tree.superseded += 1;
            self.tree.coalesced += 1;
            self.add_ledger(&previous_cause, "superseded", 1, "");
            self.add_ledger(cause, "coalesced", 1, "");
        }
        let key = format!("announce:{from}:{to}");
        let event_id = self.scheduler.schedule_coalesced(
            key,
            due,
            parent,
            SimEvent::AnnounceDue {
                from,
                to,
                cause: cause.to_owned(),
            },
        )?;
        self.pending.insert(
            (from, to),
            PendingAnnounce {
                event_id,
                cause: cause.to_owned(),
            },
        );
        Ok(())
    }

    fn evaluate_parent(&self, node: NodeId) -> Result<Option<Transition>, RunError> {
        let own_address = self.graph.address(node)?;
        let mut candidates = self
            .graph
            .active_neighbors(node)
            .into_iter()
            .filter_map(|peer| {
                self.peer_views
                    .get(&(node, peer))
                    .filter(|snapshot| !snapshot.ancestry.contains(&node))
                    .map(|snapshot| (peer, snapshot))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(None);
        }
        candidates.sort_by_key(|(peer, snapshot)| {
            (
                snapshot.root_address,
                snapshot.ancestry.len(),
                self.graph.address(*peer).ok(),
                *peer,
            )
        });
        let (best_peer, best) = candidates[0];
        if own_address <= best.root_address {
            if self.graph.parent(node).is_some() || self.graph.root(node) != node {
                return Ok(Some(Transition {
                    parent: None,
                    ancestry: vec![node],
                    mandatory: true,
                }));
            }
            return Ok(None);
        }
        let mut ancestry = Vec::with_capacity(best.ancestry.len() + 1);
        ancestry.push(node);
        ancestry.extend_from_slice(&best.ancestry);
        if self.graph.ancestry(node) == ancestry {
            return Ok(None);
        }
        let current_root_address = self.graph.address(self.graph.root(node))?;
        let mandatory = best.root_address < current_root_address
            || self.graph.parent(node).is_none()
            || self
                .graph
                .parent(node)
                .is_some_and(|parent| !self.graph.is_active(parent));
        if !mandatory {
            let hold_down = self.last_parent_switch_ns[node as usize].is_some_and(|last| {
                self.scheduler.now_ns().saturating_sub(last) < self.config.parent_hold_down_ns
            });
            if hold_down {
                return Ok(None);
            }
            let current_depth = self.graph.ancestry(node).len() as u64;
            let candidate_depth = ancestry.len() as u64;
            let threshold = current_depth.saturating_mul(
                1_000_000_u64.saturating_sub(u64::from(self.config.parent_hysteresis_ppm)),
            ) / 1_000_000;
            if candidate_depth >= threshold {
                return Ok(None);
            }
        }
        Ok(Some(Transition {
            parent: Some(best_peer),
            ancestry,
            mandatory,
        }))
    }

    fn apply_transition(
        &mut self,
        node: NodeId,
        transition: Transition,
        cause: &str,
        parent_event: Option<EventId>,
        evidence: &str,
    ) -> Result<(), RunError> {
        let old_parent = self.graph.parent(node);
        self.graph
            .set_tree(node, transition.parent, transition.ancestry)?;
        self.last_parent_switch_ns[node as usize] = Some(self.scheduler.now_ns());
        if old_parent != self.graph.parent(node) {
            self.parent_transitions += 1;
        }
        let root_address = self.graph.address(self.graph.root(node))?;
        self.root_generations.insert(root_address);
        self.add_ledger(cause, "state-mutated", 1, evidence);
        self.request_all(node, cause, parent_event)?;
        let _mandatory = transition.mandatory;
        Ok(())
    }

    fn snapshot(&self, node: NodeId) -> Result<TreeSnapshot, RunError> {
        let root = self.graph.root(node);
        Ok(TreeSnapshot {
            root,
            root_address: self.graph.address(root)?,
            parent: self.graph.parent(node),
            sequence: self.graph.sequence(node),
            ancestry: self.graph.ancestry(node).to_vec(),
        })
    }

    fn snapshot_semantics_valid(&self, snapshot: &TreeSnapshot) -> Result<bool, RunError> {
        let unique = snapshot.ancestry.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != snapshot.ancestry.len()
            || snapshot.ancestry.last() != Some(&snapshot.root)
            || snapshot.parent != snapshot.ancestry.get(1).copied()
        {
            return Ok(false);
        }
        let minimum = snapshot
            .ancestry
            .iter()
            .map(|node| self.graph.address(*node))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min();
        Ok(minimum == Some(snapshot.root_address))
    }

    fn add_ledger(&mut self, cause: &str, stage: &str, count: u64, evidence: &str) {
        let entry = self
            .ledger
            .entry((cause.to_owned(), stage.to_owned()))
            .or_default();
        entry.count = entry.count.saturating_add(count);
        if !evidence.is_empty() {
            entry.evidence.push(evidence.to_owned());
        }
    }

    fn finish(mut self) -> Result<IndividualRun, RunError> {
        if !self.pending.is_empty() || !self.scheduler.is_empty() {
            return Err(RunError::Invariant(
                "quiescence reached with pending events".to_owned(),
            ));
        }
        self.links.reconcile()?;
        let assertions = self.evaluate_invariants()?;
        if let Some(failed) = assertions.iter().find(|result| result.outcome != "pass") {
            return Err(RunError::Invariant(format!(
                "{}: {}",
                failed.id, failed.message
            )));
        }
        let fidelity = FidelityContract {
            wire: WireFidelity::ExecutableCodec,
            protocol: ProtocolFidelity::SemanticExact,
            compute: ComputeFidelity::OperationCounted,
            scale: ScaleFidelity::Individual,
            bloom: BloomFidelity::ExactBits,
            represented_nodes: self.config.nodes.into(),
            approximations: Vec::new(),
            sampled_regions: Vec::new(),
        };
        let normalized_bytes = self.plan.to_canonical_json()?;
        let normalized_sha = hex::encode(Sha256::digest(&normalized_bytes));
        let mut schema_versions = BTreeMap::new();
        schema_versions.insert(
            "normalized-plan".to_owned(),
            NORMALIZED_PLAN_VERSION.to_owned(),
        );
        schema_versions.insert("run-artifact".to_owned(), RUN_ARTIFACT_VERSION.to_owned());
        schema_versions.insert(
            "reproduction-bundle".to_owned(),
            REPRODUCTION_BUNDLE_VERSION.to_owned(),
        );
        let provenance = ProvenanceEnvelope {
            engine_name: ENGINE_NAME.to_owned(),
            engine_version: ENGINE_VERSION.to_owned(),
            engine_source_revision: engine_source_revision(),
            schema_versions,
            seed: self.plan.seed,
            normalized_plan_sha256: normalized_sha,
            fips_commit: Some(FIPS_COMMIT.to_owned()),
            image_digest: None,
            hardware_profile: None,
        };
        let run_hash_input = serde_json::to_vec(&json!({
            "plan": self.plan,
            "trace": self.trace,
            "variant": "fips-80c956a-baseline"
        }))?;
        let run_hash = hex::encode(Sha256::digest(run_hash_input));
        let run_id = format!("run-{}", &run_hash[..24]);
        let artifact_id = format!("artifact-{}", &run_hash[..32]);
        let final_root = self.minimum_active_address()?.to_hex();
        let ledger = std::mem::take(&mut self.ledger)
            .into_iter()
            .map(|((causal_id, stage), accumulator)| LedgerEntry {
                causal_id,
                stage,
                count: accumulator.count,
                evidence: accumulator.evidence,
            })
            .collect::<Vec<_>>();
        let maximum_depth = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .map(|id| self.graph.ancestry(id).len().saturating_sub(1) as u64)
            .max()
            .unwrap_or(0);
        let report = RootRatchetReport {
            kind: "root-ratchet-report/v1alpha1".to_owned(),
            run_id: run_id.clone(),
            seed: self.plan.seed,
            upstream_fips_commit: FIPS_COMMIT.to_owned(),
            fidelity_statement: fidelity.plain_language_statement(),
            graph_sha256: self.graph.graph_sha256(),
            node_count: self.config.nodes.into(),
            arrivals: self.accepted_arrivals,
            identity_generation_trials: self.identity_trials,
            final_root,
            root_generations: self
                .root_generations
                .iter()
                .copied()
                .map(NodeAddress::to_hex)
                .collect(),
            maximum_depth,
            parent_transitions: self.parent_transitions,
            quiescence_ns: self.scheduler.now_ns(),
            tree_announce: self.tree.clone(),
            links: self.links.all_counters(),
            scheduler: self.scheduler.diagnostics().clone(),
            graph_memory: self.graph.memory_footprint(),
            assertions: assertions.clone(),
        };
        let metric_series = vec![
            metric(
                "root.generations",
                "count",
                self.scheduler.now_ns(),
                report.root_generations.len() as u64,
            ),
            metric(
                "tree.maximum-depth",
                "edges",
                self.scheduler.now_ns(),
                maximum_depth,
            ),
            metric(
                "tree.parent-transitions",
                "count",
                self.scheduler.now_ns(),
                self.parent_transitions,
            ),
            metric(
                "tree-announce.transmitted-bytes",
                "bytes",
                self.scheduler.now_ns(),
                self.tree.transmitted_frame_bytes,
            ),
            metric(
                "quiescence",
                "nanoseconds",
                self.scheduler.now_ns(),
                self.scheduler.now_ns(),
            ),
        ];
        let artifact = RunArtifact {
            manifest: RunManifest {
                api_version: RUN_ARTIFACT_VERSION.to_owned(),
                artifact_id,
                run_id: run_id.clone(),
                fidelity: fidelity.clone(),
                provenance: provenance.clone(),
            },
            normalized_plan: serde_json::to_value(&self.plan)?,
            event_trace: self.trace,
            metric_series,
            causal_ledger: ledger,
            assertion_results: assertions,
            samples: vec![serde_json::to_value(&report)?],
            logs: Vec::new(),
            external_blobs: Vec::new(),
        };
        artifact.validate()?;
        let reproduction = ReproductionBundle {
            api_version: REPRODUCTION_BUNDLE_VERSION.to_owned(),
            bundle_id: format!("bundle-{}", &run_hash[..32]),
            normalized_plan: serde_json::to_value(&self.plan)?,
            seed: self.plan.seed,
            engine: ENGINE_NAME.to_owned(),
            variant: "fips-80c956a-baseline".to_owned(),
            fidelity,
            provenance,
            expected_assertions: report
                .assertions
                .iter()
                .map(|assertion| assertion.id.clone())
                .collect(),
            external_blobs: Vec::new(),
        };
        Ok(IndividualRun {
            artifact,
            reproduction,
            report,
        })
    }

    fn evaluate_invariants(&self) -> Result<Vec<AssertionResult>, RunError> {
        let active = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .collect::<Vec<_>>();
        let minimum = self.minimum_active_address()?;
        let root_agreement = active.iter().all(|node| {
            self.graph
                .address(self.graph.root(*node))
                .is_ok_and(|address| address == minimum)
        });
        let loop_free = active.iter().all(|node| {
            let path = self.graph.ancestry(*node);
            path.iter().copied().collect::<BTreeSet<_>>().len() == path.len()
        });
        let coordinate_consistent = active.iter().all(|node| {
            let path = self.graph.ancestry(*node);
            path.first() == Some(node)
                && path.last() == Some(&self.graph.root(*node))
                && self.graph.parent(*node) == path.get(1).copied()
        });
        let debounce = self.sent_times.values().all(|times| {
            times
                .windows(2)
                .all(|pair| pair[1].saturating_sub(pair[0]) >= self.config.debounce_ns)
        });
        let queues = self.links.all_counters().values().all(|counters| {
            counters.transmitted_bytes == counters.delivered_bytes + counters.lost_bytes
        });
        let lifecycle = self.tree.requested
            == self.tree.constructed + self.tree.superseded + self.tree.cancelled
            && self.tree.superseded == self.tree.coalesced
            && self.tree.constructed == self.tree.serialized
            && self.tree.constructed == self.tree.queued + self.tree.rejected;
        let checks = [
            (
                "root-agreement",
                root_agreement,
                "all active nodes advertise the minimum active address",
            ),
            (
                "loop-freedom",
                loop_free,
                "every ancestry contains unique stable node IDs",
            ),
            (
                "no-obsolete-root-retention",
                root_agreement,
                "no active node retains a superseded root at quiescence",
            ),
            (
                "per-peer-debounce",
                debounce,
                "every transmitted per-peer announcement obeys the configured boundary",
            ),
            (
                "coordinate-consistency",
                coordinate_consistent,
                "parent, root, and ancestry columns agree",
            ),
            (
                "control-queues-return-to-baseline",
                queues,
                "all transmitted bytes are delivered or deterministically lost",
            ),
            (
                "tree-lifecycle-reconciliation",
                lifecycle,
                "requested, coalesced, cancelled, constructed, serialized, queued, and rejected totals reconcile",
            ),
            (
                "byte-reconciliation",
                queues,
                "per-edge transmitted bytes equal delivered plus lost bytes",
            ),
            (
                "deterministic-total-order",
                self.trace.windows(2).all(|pair| {
                    (pair[0].virtual_time_ns, pair[0].ordinal, &pair[0].event_id)
                        <= (pair[1].virtual_time_ns, pair[1].ordinal, &pair[1].event_id)
                }),
                "event order is a stable virtual-time and ordinal total order",
            ),
        ];
        Ok(checks
            .into_iter()
            .map(|(id, passed, message)| AssertionResult {
                id: id.to_owned(),
                outcome: if passed { "pass" } else { "fail" }.to_owned(),
                message: message.to_owned(),
            })
            .collect())
    }

    fn minimum_active_address(&self) -> Result<NodeAddress, RunError> {
        self.graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .map(|id| self.graph.address(id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min()
            .ok_or_else(|| RunError::Invariant("no active nodes".to_owned()))
    }
}

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
    hasher.update(include_bytes!("graph.rs"));
    hasher.update(include_bytes!("network.rs"));
    hasher.update(include_bytes!("scheduler.rs"));
    hasher.update(include_bytes!("../Cargo.toml"));
    hex::encode(hasher.finalize())[..40].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fips_model::normalize_str;

    fn campaign(interval: &str, broken: bool) -> NormalizedPlan {
        let fault = if broken {
            "events: [{id: break, at: 1s, action: inject-parent-loop}]"
        } else {
            "events: [{id: arrivals, action: introduce-lower-root-identities}]"
        };
        normalize_str(&format!(
            r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {{name: m1-test}}
seed: 7
engine: {{modes: compact-discrete-event, deterministic: true}}
scale: {{nodes: 8}}
topology: {{generator: chain, average_degree: 2}}
identities:
  initial: {{distribution: uniform-128}}
  arrivals:
    count: 2
    schedule: {{start: 1s, interval: {interval}}}
    address_policy: strictly-lower-than-current-root
    attachment: current-root
    attacker_budget: {{mode: bounded, operations: 2}}
transports: {{assignment: all-udp}}
links: {{latency: 1ms, bandwidth_bps: 1000000000, loss_ppm: 0, ordering: stream, mtu_bytes: 9000, queue_bytes: 1048576}}
resources: {{assignment: uniform}}
{fault}
protocol: {{variant: fips-80c956a-baseline, parameters: {{tree_announce_debounce: 500ms}}}}
traffic: {{model: idle}}
fidelity: {{protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}}
accounting: {{causal_lineage: true, reconcile_serialized_frames: true}}
instrumentation: {{transition_stages: true}}
assertions: []
objectives: {{maximize: [control-bytes]}}
"#
        ))
        .unwrap()
    }

    #[test]
    fn same_seed_is_byte_identical_and_invariants_pass() {
        let plan = campaign("500ms", false);
        let left = IndividualEngine.run_plan(&plan).unwrap();
        let right = IndividualEngine.run_plan(&plan).unwrap();
        assert_eq!(
            left.artifact.to_canonical_json().unwrap(),
            right.artifact.to_canonical_json().unwrap()
        );
        assert!(
            left.report
                .assertions
                .iter()
                .all(|assertion| assertion.outcome == "pass")
        );
        assert_eq!(left.report.final_root, left.report.root_generations[0]);
    }

    #[test]
    fn debounce_boundaries_are_enforced() {
        for interval in ["499ms", "500ms", "501ms"] {
            let run = IndividualEngine
                .run_plan(&campaign(interval, false))
                .unwrap();
            assert!(
                run.report
                    .assertions
                    .iter()
                    .find(|assertion| assertion.id == "per-peer-debounce")
                    .is_some_and(|assertion| assertion.outcome == "pass")
            );
        }
    }

    #[test]
    fn deliberately_broken_fixture_fails_loud() {
        let error = IndividualEngine
            .run_plan(&campaign("500ms", true))
            .unwrap_err();
        assert!(error.to_string().contains("loop-freedom"));
    }

    #[test]
    fn bounded_identity_generation_exhausts_deterministically() {
        let mut plan = campaign("500ms", false);
        *plan
            .campaign
            .pointer_mut("/identities/arrivals/attacker_budget/operations")
            .unwrap() = Value::from(1);
        assert!(matches!(
            IndividualEngine.run_plan(&plan),
            Err(RunError::BudgetExhausted { .. })
        ));
    }

    #[test]
    fn precomputed_ladder_is_validated_and_charges_no_grinding_trials() {
        let mut plan = campaign("500ms", false);
        let arrivals = plan
            .campaign
            .pointer_mut("/identities/arrivals")
            .and_then(Value::as_object_mut)
            .unwrap();
        arrivals.insert(
            "address_policy".to_owned(),
            Value::String("precomputed-ladder".to_owned()),
        );
        arrivals.insert(
            "precomputed_ladder".to_owned(),
            json!([
                "7fffffffffffffffffffffffffffff00",
                "7ffffffffffffffffffffffffffffe00"
            ]),
        );
        *plan
            .campaign
            .pointer_mut("/identities/arrivals/attacker_budget/operations")
            .unwrap() = Value::from(0);
        let run = IndividualEngine.run_plan(&plan).unwrap();
        assert_eq!(run.report.identity_generation_trials, 0);
        assert_eq!(run.report.final_root, "7ffffffffffffffffffffffffffffe00");
    }

    #[test]
    fn disappearance_and_reappearance_reconverge() {
        let mut plan = campaign("500ms", false);
        *plan.campaign.pointer_mut("/events").unwrap() = json!([
            {"id": "down", "at": {"nanoseconds": 2_000_000_000_u64}, "action": "disappear-node", "target": 7},
            {"id": "up", "at": {"nanoseconds": 3_000_000_000_u64}, "action": "reappear-node", "target": 7}
        ]);
        let run = IndividualEngine.run_plan(&plan).unwrap();
        assert!(
            run.artifact
                .event_trace
                .iter()
                .any(|event| event.kind == "input.node-disappeared")
        );
        assert!(
            run.artifact
                .event_trace
                .iter()
                .any(|event| event.kind == "input.node-reappeared")
        );
        assert!(
            run.report
                .assertions
                .iter()
                .all(|assertion| assertion.outcome == "pass")
        );
    }

    #[test]
    fn better_root_bypasses_parent_hold_down() {
        let mut plan = campaign("500ms", false);
        let parameters = plan
            .campaign
            .pointer_mut("/protocol/parameters")
            .and_then(Value::as_object_mut)
            .unwrap();
        parameters.insert(
            "parent_hold_down".to_owned(),
            json!({"nanoseconds": 10_000_000_000_u64}),
        );
        let run = IndividualEngine.run_plan(&plan).unwrap();
        assert_eq!(run.report.final_root, run.report.root_generations[0]);
        assert!(
            run.report
                .assertions
                .iter()
                .find(|assertion| assertion.id == "root-agreement")
                .is_some_and(|assertion| assertion.outcome == "pass")
        );
    }

    #[test]
    fn current_fips_pin_is_exact() {
        assert_eq!(FIPS_COMMIT, fips_artifact::M0_FIPS_COMMIT);
    }
}
