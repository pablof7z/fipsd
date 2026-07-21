//! Virtual-time resource service with typed causal exhaustion.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Modeled work or state dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    /// Signature generation.
    Signatures,
    /// Signature verification.
    Verifications,
    /// Hash invocations.
    Hashes,
    /// Bloom hash/OR operations.
    BloomOperations,
    /// Allocation bytes.
    AllocationBytes,
    /// Coordinate cache entries.
    CacheEntries,
    /// Peer table entries.
    Peers,
    /// Active sessions.
    Sessions,
    /// Handshake work.
    Handshakes,
    /// Connection slots.
    Connections,
    /// Queue bytes.
    QueueBytes,
}

impl ResourceKind {
    /// Whether this dimension consumes virtual CPU service time.
    pub fn is_service(self) -> bool {
        matches!(
            self,
            Self::Signatures
                | Self::Verifications
                | Self::Hashes
                | Self::BloomOperations
                | Self::Handshakes
        )
    }
}

/// One heterogeneous node resource profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceProfile {
    /// Stable profile name.
    pub name: String,
    /// CPU work units serviced per millisecond.
    pub cpu_units_per_ms: u64,
    /// Hard capacities by resource kind.
    pub capacities: BTreeMap<ResourceKind, u64>,
    /// Deliberate scheduler pauses as `[start,end)` nanoseconds.
    pub pauses: Vec<(u64, u64)>,
}

impl ResourceProfile {
    /// Baseline profile with explicit capacities.
    pub fn baseline() -> Self {
        let capacities = [
            (ResourceKind::AllocationBytes, 1 << 30),
            (ResourceKind::CacheEntries, 100_000),
            (ResourceKind::Peers, 10_000),
            (ResourceKind::Sessions, 100_000),
            (ResourceKind::Connections, 10_000),
            (ResourceKind::QueueBytes, 1 << 20),
        ]
        .into_iter()
        .collect();
        Self {
            name: "baseline".to_owned(),
            cpu_units_per_ms: 1_000,
            capacities,
            pauses: Vec::new(),
        }
    }
}

/// Completed virtual-time resource service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceReceipt {
    /// Stable causal owner.
    pub causal_id: String,
    /// Node receiving service.
    pub node: u32,
    /// Work/state kind.
    pub kind: ResourceKind,
    /// Units consumed.
    pub units: u64,
    /// Requested virtual time.
    pub requested_at_ns: u64,
    /// Service start after prior work and pauses.
    pub started_at_ns: u64,
    /// Completion time.
    pub completed_at_ns: u64,
}

/// Aggregate resource diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceCounters {
    /// Work/state units consumed by kind.
    pub consumed: BTreeMap<ResourceKind, u64>,
    /// CPU queue wait across receipts.
    pub queue_wait_ns: u64,
    /// Longest CPU queue wait.
    pub maximum_queue_wait_ns: u64,
    /// Typed exhaustion attempts.
    pub exhaustion_count: u64,
}

/// Per-node resource service.
#[derive(Debug, Clone)]
pub struct ResourcePool {
    profile: ResourceProfile,
    cpu_available_ns: u64,
    retained: BTreeMap<ResourceKind, u64>,
    /// Receipts proving every executed operation consumed a budget.
    pub receipts: Vec<ResourceReceipt>,
    /// Aggregate counters.
    pub counters: ResourceCounters,
}

impl ResourcePool {
    /// Create a pool for one node profile.
    pub fn new(profile: ResourceProfile) -> Self {
        Self {
            profile,
            cpu_available_ns: 0,
            retained: BTreeMap::new(),
            receipts: Vec::new(),
            counters: ResourceCounters::default(),
        }
    }

    /// Consume service work or retain state and return a causal receipt.
    pub fn consume(
        &mut self,
        causal_id: impl Into<String>,
        node: u32,
        kind: ResourceKind,
        units: u64,
        requested_at_ns: u64,
    ) -> Result<ResourceReceipt, ResourceError> {
        let causal_id = causal_id.into();
        if !kind.is_service() {
            let current = self.retained.get(&kind).copied().unwrap_or(0);
            let available = self.profile.capacities.get(&kind).copied().unwrap_or(0);
            if current.saturating_add(units) > available {
                self.counters.exhaustion_count += 1;
                return Err(ResourceError::Exhausted {
                    causal_id,
                    node,
                    kind,
                    requested: current.saturating_add(units),
                    available,
                });
            }
            self.retained.insert(kind, current + units);
        }
        let mut start = requested_at_ns.max(self.cpu_available_ns);
        if kind.is_service() {
            start = after_pauses(start, &self.profile.pauses);
        }
        let duration_ns = if kind.is_service() {
            if self.profile.cpu_units_per_ms == 0 {
                self.counters.exhaustion_count += 1;
                return Err(ResourceError::NoService {
                    causal_id,
                    node,
                    kind,
                });
            }
            units
                .saturating_mul(1_000_000)
                .div_ceil(self.profile.cpu_units_per_ms)
        } else {
            0
        };
        let completed = after_pauses(start.saturating_add(duration_ns), &self.profile.pauses);
        if kind.is_service() {
            self.cpu_available_ns = completed;
        }
        let wait = start.saturating_sub(requested_at_ns);
        self.counters.queue_wait_ns = self.counters.queue_wait_ns.saturating_add(wait);
        self.counters.maximum_queue_wait_ns = self.counters.maximum_queue_wait_ns.max(wait);
        *self.counters.consumed.entry(kind).or_default() += units;
        let receipt = ResourceReceipt {
            causal_id,
            node,
            kind,
            units,
            requested_at_ns,
            started_at_ns: start,
            completed_at_ns: completed,
        };
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }

    /// Release retained state.
    pub fn release(&mut self, kind: ResourceKind, units: u64) {
        if let Some(retained) = self.retained.get_mut(&kind) {
            *retained = retained.saturating_sub(units);
        }
    }

    /// Current retained state.
    pub fn retained(&self, kind: ResourceKind) -> u64 {
        self.retained.get(&kind).copied().unwrap_or(0)
    }
}

/// Resource service failure with causal context.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceError {
    /// A hard capacity was exceeded.
    #[error(
        "resource exhausted for {causal_id} on node {node}: {kind:?} requested {requested}, available {available}"
    )]
    Exhausted {
        /// Initiating cause.
        causal_id: String,
        /// Node.
        node: u32,
        /// Resource.
        kind: ResourceKind,
        /// Requested total.
        requested: u64,
        /// Capacity.
        available: u64,
    },
    /// CPU class cannot service work.
    #[error("resource has no service capacity for {causal_id} on node {node}: {kind:?}")]
    NoService {
        /// Initiating cause.
        causal_id: String,
        /// Node.
        node: u32,
        /// Resource.
        kind: ResourceKind,
    },
}

fn after_pauses(mut at_ns: u64, pauses: &[(u64, u64)]) -> u64 {
    for (start, end) in pauses {
        if at_ns >= *start && at_ns < *end {
            at_ns = *end;
        }
    }
    at_ns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_modeled_work_item_has_a_receipt_and_competes_for_cpu() {
        let mut pool = ResourcePool::new(ResourceProfile::baseline());
        let bloom = pool
            .consume("arrival:1", 0, ResourceKind::BloomOperations, 1_000, 0)
            .unwrap();
        let data = pool
            .consume("flow:1", 0, ResourceKind::Hashes, 1_000, 0)
            .unwrap();
        assert_eq!(bloom.completed_at_ns, 1_000_000);
        assert_eq!(data.started_at_ns, bloom.completed_at_ns);
        assert_eq!(pool.receipts.len(), 2);
        assert_eq!(pool.counters.queue_wait_ns, 1_000_000);
    }

    #[test]
    fn exhaustion_is_typed_and_causal() {
        let mut profile = ResourceProfile::baseline();
        profile.capacities.insert(ResourceKind::Sessions, 1);
        let mut pool = ResourcePool::new(profile);
        pool.consume("flow:1", 3, ResourceKind::Sessions, 1, 0)
            .unwrap();
        let error = pool
            .consume("flow:2", 3, ResourceKind::Sessions, 1, 0)
            .unwrap_err();
        assert!(matches!(
            error,
            ResourceError::Exhausted {
                causal_id,
                node: 3,
                kind: ResourceKind::Sessions,
                ..
            } if causal_id == "flow:2"
        ));
    }

    #[test]
    fn slow_and_paused_profiles_expose_expected_bottleneck() {
        let mut slow = ResourceProfile::baseline();
        slow.cpu_units_per_ms = 1;
        slow.pauses = vec![(500_000, 5_000_000)];
        let mut pool = ResourcePool::new(slow);
        let receipt = pool
            .consume("root:slow", 0, ResourceKind::Signatures, 2, 0)
            .unwrap();
        assert_eq!(receipt.completed_at_ns, 5_000_000);
    }
}
