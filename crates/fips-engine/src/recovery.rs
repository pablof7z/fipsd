//! Coupled M2 control/data-plane recovery model and causal report.

use crate::bloom::{
    BloomMode, BloomModel, BloomReplacementWave, BloomWaveCounters, FILTER_ANNOUNCE_FMP_BYTES,
    PeerRole,
};
use crate::cache::{CacheCounters, CoordinateCache, Invalidation};
use crate::engine::RootRatchetReport;
use crate::lookup::{
    ESTABLISHED_FMP_OVERHEAD_BYTES, LookupCase, LookupConfig, LookupCounters, LookupOutcome,
    LookupService, RoutingSignal,
};
use crate::network::{
    EnqueueRequest, LinkClass, LinkConfig, LinkCounters, LinkOrdering, LinkService,
};
use crate::resources::{
    ResourceCounters, ResourceError, ResourceKind, ResourcePool, ResourceProfile,
};
use crate::traffic::{SessionAction, TrafficConfig, TrafficModel, TrafficPlan};
use fips_artifact::{
    Approximation, AssertionResult, BloomFidelity, LedgerEntry, MetricPoint, MetricSeries,
};
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use self::helpers::*;

const DATA_FRAME_OVERHEAD_BYTES: u64 = 106;
const DEFAULT_BLOOM_DEBOUNCE_NS: u64 = 500_000_000;

/// Quiescence points across the coupled recovery path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryMarkers {
    /// Root agreement.
    pub root_ns: u64,
    /// TreeAnnounce quiescence.
    pub tree_ns: u64,
    /// Bloom replacement quiescence.
    pub bloom_ns: u64,
    /// Last lookup recovery.
    pub lookup_ns: u64,
    /// Last useful delivery.
    pub throughput_ns: u64,
}

/// Useful traffic result, kept distinct from framing/network bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficRecovery {
    /// Offered flows.
    pub offered_flows: u64,
    /// Delivered flows.
    pub delivered_flows: u64,
    /// Starved/rejected/lost flows.
    pub starved_flows: u64,
    /// Application bytes offered.
    pub offered_useful_bytes: u64,
    /// Application bytes delivered.
    pub delivered_useful_bytes: u64,
    /// Application bytes lost/rejected.
    pub lost_useful_bytes: u64,
    /// Session setups.
    pub session_setups: u64,
    /// Session teardowns.
    pub session_teardowns: u64,
    /// Rekey hooks.
    pub rekeys: u64,
    /// Exact setup message bytes.
    pub setup_message_bytes: u64,
    /// Maximum end-to-end modeled flow latency.
    pub maximum_latency_ns: u64,
    /// Cumulative delivered-flow latency.
    pub total_latency_ns: u64,
    /// Longest interval after the first arrival without a useful delivery.
    pub goodput_stall_ns: u64,
}

/// Ledger-layer totals and exact reconciliation flags.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayeredCosts {
    /// Semantic actions.
    pub semantic_actions: u64,
    /// Application payload offered.
    pub payload_bytes: u64,
    /// FSP/session/lookup message bytes.
    pub fsp_bytes: u64,
    /// FMP frame bytes accepted by the shared service.
    pub fmp_bytes: u64,
    /// Transport header bytes.
    pub transport_bytes: u64,
    /// Total transmitted network bytes including duplicates.
    pub network_bytes: u64,
    /// Loss/duplication bytes.
    pub reliability_bytes: u64,
    /// Useful application bytes delivered.
    pub useful_payload_bytes: u64,
    /// Modeled work units.
    pub compute_units: u64,
    /// Peak modeled Bloom/cache state.
    pub state_bytes: u64,
    /// Critical-path virtual time.
    pub time_ns: u64,
    /// Irreducible accepted frame bytes under this schedule.
    pub lower_bound_bytes: u64,
    /// Duplicate wire bytes.
    pub duplicate_bytes: u64,
    /// Coalesced/superseded replacement bytes avoided.
    pub superseded_bytes: u64,
    /// Retransmitted/duplicate wire bytes.
    pub retransmitted_bytes: u64,
    /// Network/useful ratio in parts per million.
    pub amplification_ppm: u64,
    /// Frame records sum exactly to accepted frame bytes.
    pub frames_reconcile: bool,
    /// Aggregate equals message and edge projections.
    pub projections_reconcile: bool,
    /// Ledger layers equal their displayed aggregate totals.
    pub ledger_reconcile: bool,
}

/// Projection totals proving aggregation paths agree.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostProjections {
    /// Accepted frame bytes by message type.
    pub by_message: BTreeMap<String, u64>,
    /// Transmitted wire bytes by modeled bottleneck edge.
    pub by_edge: BTreeMap<String, u64>,
    /// Modeled work by resource class.
    pub by_resource: BTreeMap<ResourceKind, u64>,
    /// Accepted frame bytes by path-depth band.
    pub by_depth_band: BTreeMap<String, u64>,
}

/// Per-arrival causal amplification summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrivalAmplification {
    /// Root arrival causal ID.
    pub causal_id: String,
    /// Arrival time.
    pub at_ns: u64,
    /// Bloom replacement requests.
    pub bloom_requests: u64,
    /// Bloom frames attributed to the wave.
    pub bloom_frames: u64,
    /// Bloom full-replacement bytes.
    pub bloom_fmp_bytes: u64,
    /// Coordinate entries invalidated.
    pub cache_invalidations: u64,
    /// CPU work charged.
    pub compute_units: u64,
}

/// Critical-path explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriticalPath {
    /// Dominant modeled component.
    pub component: String,
    /// Dominant duration.
    pub duration_ns: u64,
    /// Plain-language causal explanation.
    pub explanation: String,
    /// Ledger causal IDs supporting the explanation.
    pub evidence: Vec<String>,
}

/// Full M2 Root Ratchet recovery and starvation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReport {
    /// Report discriminator.
    pub kind: String,
    /// Parent M1 run ID.
    pub run_id: String,
    /// Bloom representation, never implicit.
    pub bloom_mode: BloomMode,
    /// Plain-language fidelity declaration.
    pub fidelity_statement: String,
    /// Quiescence points.
    pub markers: RecoveryMarkers,
    /// Root generations traversed.
    pub intermediate_roots: u64,
    /// Adoption times by deterministic depth bands.
    pub depth_band_adoption_ns: BTreeMap<String, u64>,
    /// Split-horizon Bloom accounting.
    pub bloom: BloomWaveCounters,
    /// Final Bloom fill ratio in parts per million.
    pub bloom_fill_ppm: u64,
    /// Final Bloom FPR in parts per billion.
    pub bloom_fpr_ppb: u64,
    /// Bloom cardinality estimate rounded to elements.
    pub bloom_estimated_cardinality: Option<u64>,
    /// Coordinate cache behavior.
    pub cache: CacheCounters,
    /// Lookup and retry behavior.
    pub lookup: LookupCounters,
    /// Useful traffic behavior.
    pub traffic: TrafficRecovery,
    /// Aggregate node-resource behavior.
    pub resources: ResourceCounters,
    /// Typed causal exhaustion outcomes.
    pub resource_exhaustions: Vec<ResourceError>,
    /// Shared bottleneck counters.
    pub shared_link: LinkCounters,
    /// Maximum accepted frame bytes.
    pub maximum_frame_bytes: u64,
    /// Peak shared queue occupancy.
    pub peak_queue_bytes: u64,
    /// Control/useful delivered ratio in ppm.
    pub control_to_useful_ppm: u64,
    /// Ledger layers.
    pub costs: LayeredCosts,
    /// Cross-dimensional projections.
    pub projections: CostProjections,
    /// Root-arrival summaries.
    pub per_arrival: Vec<ArrivalAmplification>,
    /// Dominant causal path.
    pub critical_path: CriticalPath,
    /// M2 assertions.
    pub assertions: Vec<AssertionResult>,
    /// Artifact-ready causal rows.
    pub causal_ledger: Vec<LedgerEntry>,
}

impl RecoveryReport {
    /// Artifact metric series.
    pub fn metric_series(&self) -> Vec<MetricSeries> {
        [
            ("quiescence.root", "nanoseconds", self.markers.root_ns),
            ("quiescence.tree", "nanoseconds", self.markers.tree_ns),
            ("quiescence.bloom", "nanoseconds", self.markers.bloom_ns),
            ("quiescence.lookup", "nanoseconds", self.markers.lookup_ns),
            (
                "quiescence.data-plane",
                "nanoseconds",
                self.markers.throughput_ns,
            ),
            (
                "traffic.useful-delivered",
                "bytes",
                self.traffic.delivered_useful_bytes,
            ),
            ("cache.invalidations", "count", self.cache.invalidations),
            ("bloom.fpr", "parts-per-billion", self.bloom_fpr_ppb),
            (
                "resource.maximum-queue-wait",
                "nanoseconds",
                self.resources.maximum_queue_wait_ns,
            ),
        ]
        .into_iter()
        .map(|(name, unit, value)| MetricSeries {
            name: name.to_owned(),
            unit: unit.to_owned(),
            points: vec![MetricPoint {
                virtual_time_ns: self.markers.throughput_ns.max(self.markers.bloom_ns),
                value: value.to_string(),
            }],
        })
        .collect()
    }

    /// Artifact Bloom fidelity and approximation metadata.
    pub fn fidelity(&self) -> (BloomFidelity, Vec<Approximation>) {
        match self.bloom_mode {
            BloomMode::ExactBits => (BloomFidelity::ExactBits, Vec::new()),
            BloomMode::SparseBits => (BloomFidelity::SparseBits, Vec::new()),
            BloomMode::Occupancy => (
                BloomFidelity::Occupancy,
                vec![Approximation {
                    method: "seeded-bloom-occupancy-v1".to_owned(),
                    parameters: [
                        ("bits".to_owned(), "8192".to_owned()),
                        ("hash-count".to_owned(), "5".to_owned()),
                    ]
                    .into_iter()
                    .collect(),
                    validated_range: "seeded ensembles through the configured FPR cap".to_owned(),
                    uncertainty: "membership of absent items is a deterministic Bernoulli draw at the modeled FPR".to_owned(),
                }],
            ),
        }
    }
}

/// Coupled M2 runner.
#[derive(Debug, Clone, Default)]
pub struct RecoveryEngine;

impl RecoveryEngine {
    /// Whether a normalized plan asks for the M2 recovery surface.
    pub fn requested(plan: &NormalizedPlan) -> bool {
        let markers = plan
            .campaign
            .pointer("/instrumentation/quiescence_markers")
            .and_then(Value::as_array);
        markers.is_some_and(|markers| {
            markers
                .iter()
                .any(|marker| matches!(marker.as_str(), Some("bloom" | "lookup" | "data-plane")))
        }) || plan
            .campaign
            .pointer("/traffic/model")
            .and_then(Value::as_str)
            .is_some_and(|model| model != "idle")
    }

    /// Run the coupled M2 model.
    pub fn run(
        &self,
        plan: &NormalizedPlan,
        root: &RootRatchetReport,
    ) -> Result<RecoveryReport, RecoveryError> {
        let config = RecoveryConfig::from_plan(plan, root)?;
        CoupledState::new(plan.seed, root, config)?.run()
    }
}

#[derive(Debug, Clone)]
struct RecoveryConfig {
    nodes: u32,
    arrivals: u32,
    arrival_start_ns: u64,
    arrival_interval_ns: u64,
    tree_recovery_ns: u64,
    bloom_mode: BloomMode,
    bloom_debounce_ns: u64,
    bloom_max_fpr: f64,
    cache_entries: usize,
    cache_ttl_ns: u64,
    lookup: LookupConfig,
    traffic: TrafficConfig,
    link: LinkConfig,
    resource_profile: ResourceProfile,
    heterogeneous: bool,
}

#[derive(Debug, Clone)]
enum WorkEvent {
    Arrival(u32),
    BloomArrival(u32),
    Flow(usize),
}

#[derive(Debug)]
struct CoupledState<'a> {
    seed: u64,
    root: &'a RootRatchetReport,
    config: RecoveryConfig,
    bloom_model: BloomModel,
    bloom_wave: BloomReplacementWave,
    cache: CoordinateCache,
    lookup: LookupService,
    traffic_plan: TrafficPlan,
    resources: Vec<ResourcePool>,
    link: LinkService,
    link_delivery_ns: u64,
    traffic: TrafficRecovery,
    lookup_counters: LookupCounters,
    resource_exhaustions: Vec<ResourceError>,
    ledger: Vec<LedgerEntry>,
    projections: CostProjections,
    accepted_frame_bytes: u64,
    logical_wire_bytes: u64,
    maximum_frame_bytes: u64,
    arrival_summaries: Vec<ArrivalAmplification>,
    current_root: [u8; 16],
    latest_arrival_cause: Option<String>,
    last_lookup_ns: u64,
    useful_delivery_times: Vec<u64>,
    last_useful_delivery_ns: Option<u64>,
}

#[path = "recovery_config.rs"]
mod config;
#[path = "recovery_finish.rs"]
mod finish;
#[path = "recovery_flow.rs"]
mod flow;
#[path = "recovery_helpers.rs"]
mod helpers;
#[path = "recovery_io.rs"]
mod io;
#[path = "recovery_runtime.rs"]
mod runtime;
#[path = "recovery_state.rs"]
mod state;

/// M2 execution failure.
#[derive(Debug, Error)]
pub enum RecoveryError {
    /// Unsupported resolved input.
    #[error("unsupported M2 recovery configuration: {0}")]
    Unsupported(String),
    /// Coupled invariant failed.
    #[error("M2 recovery invariant failed: {0}")]
    Invariant(String),
    /// Traffic generator failure.
    #[error(transparent)]
    Traffic(#[from] crate::TrafficError),
    /// Link service failure.
    #[error(transparent)]
    Link(#[from] crate::LinkError),
    /// Invalid hexadecimal root fixture.
    #[error("invalid root address {0}")]
    RootAddress(String),
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
