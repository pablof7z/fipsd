use super::*;
use crate::{
    CacheCounters, CoordinateCache, Invalidation, ResourceCounters, ResourceError, ResourceKind,
    ResourcePool, ResourceProfile,
};

#[path = "engine_resource_profiles.rs"]
mod resource_profiles;
use resource_profiles::resource_profiles;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphRecoveryCounters {
    pub cache: CacheCounters,
    pub lookups: u64,
    pub attempts: u64,
    pub retries: u64,
    pub successes: u64,
    pub failures: u64,
    pub session_setups: u64,
    pub session_acks: u64,
    pub rekeys: u64,
    pub teardowns: u64,
    pub session_disruptions: u64,
    pub transmitted_wire_bytes: u64,
    pub delivered_wire_bytes: u64,
    pub lost_wire_bytes: u64,
    pub resources: ResourceCounters,
    pub resource_exhaustions: Vec<ResourceError>,
    pub quiescence_ns: u64,
}

impl GraphRecoveryCounters {
    pub fn reconciles(&self) -> bool {
        self.lookups == self.successes + self.failures
            && self.attempts == self.lookups + self.retries
            && self.transmitted_wire_bytes == self.delivered_wire_bytes + self.lost_wire_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum RecoveryPhase {
    LookupRequest,
    LookupResponse,
    SessionSetup,
    SessionAck,
}

impl RecoveryPhase {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::LookupRequest => "lookup-request",
            Self::LookupResponse => "lookup-response",
            Self::SessionSetup => "session-setup",
            Self::SessionAck => "session-ack",
        }
    }

    pub(super) fn plane(self) -> &'static str {
        match self {
            Self::LookupRequest | Self::LookupResponse => "lookup",
            Self::SessionSetup | Self::SessionAck => "session",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RecoveryFrame {
    pub flow_index: usize,
    pub attempt: u32,
    pub phase: RecoveryPhase,
    pub path: Vec<NodeId>,
    pub hop: usize,
    pub frame_bytes: u64,
}

#[derive(Debug, Clone)]
pub(super) struct GraphRecoveryRuntime {
    caches: Vec<CoordinateCache>,
    resources: Vec<ResourcePool>,
    sessions: BTreeMap<(NodeId, NodeId), Vec<NodeId>>,
    pub ttl: u8,
    pub maximum_attempts: u32,
    pub backoff_base_ns: u64,
    pub jitter_ns: u64,
    pub counters: GraphRecoveryCounters,
}

impl GraphRecoveryRuntime {
    pub(super) fn requested(plan: &NormalizedPlan) -> bool {
        let lookup = plan
            .campaign
            .pointer("/instrumentation/quiescence_markers")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("lookup")));
        let mixed = plan
            .campaign
            .pointer("/transports/assignment")
            .and_then(Value::as_str)
            == Some("random-mixed");
        lookup && mixed
    }

    pub(super) fn from_plan(plan: &NormalizedPlan, nodes: u32) -> Result<Option<Self>, RunError> {
        if !Self::requested(plan) {
            return Ok(None);
        }
        let cache_entries = integer(plan, "/protocol/parameters/coord_cache_entries", 64)? as usize;
        let cache_ttl_ns = duration(plan, "/protocol/parameters/coord_cache_ttl", 5_000_000_000);
        let profiles = resource_profiles(plan, nodes)?;
        Ok(Some(Self {
            caches: (0..nodes)
                .map(|_| CoordinateCache::new(cache_entries, cache_ttl_ns))
                .collect(),
            resources: profiles.into_iter().map(ResourcePool::new).collect(),
            sessions: BTreeMap::new(),
            ttl: u8::try_from(integer(plan, "/protocol/parameters/lookup_ttl", 64)?)
                .map_err(|_| RunError::Unsupported("lookup TTL exceeds u8".to_owned()))?,
            maximum_attempts: u32::try_from(integer(
                plan,
                "/protocol/parameters/lookup_attempts",
                3,
            )?)
            .map_err(|_| RunError::Unsupported("lookup attempts exceed u32".to_owned()))?
            .max(1),
            backoff_base_ns: duration(plan, "/protocol/parameters/lookup_backoff", 100_000_000),
            jitter_ns: duration(plan, "/protocol/parameters/lookup_jitter", 10_000_000),
            counters: GraphRecoveryCounters::default(),
        }))
    }

    pub(super) fn snapshot_counters(&self) -> GraphRecoveryCounters {
        let mut counters = self.counters.clone();
        counters.cache = aggregate_cache(&self.caches);
        counters.resources = aggregate_resources(&self.resources);
        counters
    }

    pub(super) fn is_cached(&mut self, source: NodeId, destination: [u8; 16], now_ns: u64) -> bool {
        self.caches[source as usize]
            .get(&destination, now_ns)
            .is_some()
    }

    pub(super) fn insert_cache(
        &mut self,
        source: NodeId,
        destination: [u8; 16],
        root: [u8; 16],
        path: Vec<NodeId>,
        now_ns: u64,
    ) {
        self.caches[source as usize].insert(destination, root, path, now_ns);
    }

    pub(super) fn consume(
        &mut self,
        causal_id: &str,
        node: NodeId,
        kind: ResourceKind,
        units: u64,
        now_ns: u64,
    ) -> Result<u64, ResourceError> {
        match self.resources[node as usize].consume(causal_id, node, kind, units, now_ns) {
            Ok(receipt) => Ok(receipt.completed_at_ns),
            Err(error) => {
                self.counters.resource_exhaustions.push(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn invalidate_node(&mut self, address: [u8; 16]) -> u64 {
        self.caches
            .iter_mut()
            .map(|cache| cache.invalidate(&Invalidation::Node(address)))
            .sum()
    }

    pub(super) fn invalidate_root(&mut self, address: [u8; 16]) -> u64 {
        self.caches
            .iter_mut()
            .map(|cache| cache.invalidate(&Invalidation::Root(address)))
            .sum()
    }

    pub(super) fn invalidate_path_node(&mut self, node: NodeId) -> u64 {
        self.caches
            .iter_mut()
            .map(|cache| cache.invalidate(&Invalidation::PathNode(node)))
            .sum()
    }

    pub(super) fn invalidate_path_edge(&mut self, left: NodeId, right: NodeId) -> u64 {
        self.caches
            .iter_mut()
            .map(|cache| cache.invalidate(&Invalidation::PathEdge(left, right)))
            .sum()
    }
}

fn integer(plan: &NormalizedPlan, pointer: &str, default: u64) -> Result<u64, RunError> {
    Ok(plan
        .campaign
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or(default))
}

fn duration(plan: &NormalizedPlan, pointer: &str, default: u64) -> u64 {
    plan.campaign
        .pointer(pointer)
        .and_then(|value| value.get("nanoseconds"))
        .and_then(Value::as_u64)
        .unwrap_or(default)
}

fn aggregate_cache(caches: &[CoordinateCache]) -> CacheCounters {
    let mut total = CacheCounters::default();
    for cache in caches {
        total.insertions += cache.counters.insertions;
        total.hits += cache.counters.hits;
        total.misses += cache.counters.misses;
        total.expirations += cache.counters.expirations;
        total.evictions += cache.counters.evictions;
        total.invalidations += cache.counters.invalidations;
        total.warmup_bytes += cache.counters.warmup_bytes;
        total.peak_entries = total.peak_entries.max(cache.counters.peak_entries);
        total.peak_bytes = total.peak_bytes.max(cache.counters.peak_bytes);
    }
    total
}

fn aggregate_resources(pools: &[ResourcePool]) -> ResourceCounters {
    let mut total = ResourceCounters::default();
    for pool in pools {
        for (kind, units) in &pool.counters.consumed {
            *total.consumed.entry(*kind).or_default() += units;
        }
        total.queue_wait_ns += pool.counters.queue_wait_ns;
        total.maximum_queue_wait_ns = total
            .maximum_queue_wait_ns
            .max(pool.counters.maximum_queue_wait_ns);
        total.exhaustion_count += pool.counters.exhaustion_count;
    }
    total
}

#[path = "engine_recovery_state.rs"]
mod state;

#[cfg(test)]
#[path = "engine_recovery_tests.rs"]
mod tests;
