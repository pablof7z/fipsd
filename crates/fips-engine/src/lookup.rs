//! Deterministic lookup, retry, dedup, TTL, MTU, and routing-signal model.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;

/// Production LookupRequest bytes at coordinate depth zero.
pub const LOOKUP_REQUEST_DEPTH_0_BYTES: u64 = 62;
/// Production LookupResponse bytes at coordinate depth zero.
pub const LOOKUP_RESPONSE_DEPTH_0_BYTES: u64 = 109;
/// Established FMP overhead from the pinned executable codec.
pub const ESTABLISHED_FMP_OVERHEAD_BYTES: u64 = 36;
/// Production CoordsRequired FSP-framed bytes.
pub const COORDS_REQUIRED_BYTES: u64 = 38;
/// Production MtuExceeded FSP-framed bytes.
pub const MTU_EXCEEDED_BYTES: u64 = 40;

/// Routing signal generated while useful traffic recovers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingSignal {
    /// Coordinate cache miss.
    CoordsRequired,
    /// Stale or broken reverse/forward path.
    PathBroken,
    /// Encoded frame cannot traverse the bottleneck MTU.
    MtuExceeded,
}

/// Final lookup outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LookupOutcome {
    /// Target response reached the origin.
    Success,
    /// Hop budget ended first.
    TtlExpired,
    /// Encoded request or response exceeded MTU.
    MtuExceeded,
    /// Reverse path became invalid.
    ReversePathBroken,
    /// No candidate produced a response.
    Negative,
}

/// One retry attempt with stable lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupAttempt {
    /// Stable request identifier.
    pub request_id: u64,
    /// Stable causal identifier.
    pub causal_id: String,
    /// Prior attempt causal ID.
    pub parent_causal_id: Option<String>,
    /// Attempt ordinal.
    pub attempt: u32,
    /// Start time after backoff/jitter.
    pub started_at_ns: u64,
    /// Hop budget.
    pub ttl: u8,
    /// Required path hops.
    pub required_hops: u32,
    /// Exact encoded request bytes.
    pub request_message_bytes: u64,
    /// Exact encoded response bytes when constructed.
    pub response_message_bytes: u64,
    /// Bloom false-positive candidates explored.
    pub false_positive_candidates: u32,
    /// Duplicate forward copies suppressed.
    pub deduplicated: u32,
    /// Attempt outcome.
    pub outcome: LookupOutcome,
    /// Signal generated, if any.
    pub signal: Option<RoutingSignal>,
}

/// Aggregate lookup accounting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupCounters {
    /// Logical lookups.
    pub lookups: u64,
    /// Request attempts including retries.
    pub attempts: u64,
    /// Successful lookups.
    pub successes: u64,
    /// Failed logical lookups.
    pub failures: u64,
    /// Retry attempts after the first.
    pub retries: u64,
    /// Recent-request duplicate copies suppressed.
    pub deduplicated: u64,
    /// Bloom false-positive candidate forwards.
    pub false_positive_candidates: u64,
    /// Request message bytes.
    pub request_message_bytes: u64,
    /// Response message bytes.
    pub response_message_bytes: u64,
    /// Signal counts by kind.
    pub signals: BTreeMap<RoutingSignal, u64>,
    /// Signals suppressed by rate limiting.
    pub signals_rate_limited: u64,
    /// Time from first attempt to final success/failure.
    pub recovery_time_ns: u64,
}

/// One deterministic lookup result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupRun {
    /// Attempt lineage.
    pub attempts: Vec<LookupAttempt>,
    /// Final outcome.
    pub outcome: LookupOutcome,
    /// Reconciled counters.
    pub counters: LookupCounters,
}

/// Lookup/backoff policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupConfig {
    /// Initial TTL.
    pub ttl: u8,
    /// Total attempts.
    pub maximum_attempts: u32,
    /// Exponential backoff base.
    pub backoff_base_ns: u64,
    /// Maximum deterministic jitter.
    pub jitter_ns: u64,
    /// Recent request IDs retained.
    pub recent_capacity: usize,
    /// Minimum interval per signal kind.
    pub signal_interval_ns: u64,
    /// Bottleneck link MTU.
    pub mtu_bytes: u64,
}

impl Default for LookupConfig {
    fn default() -> Self {
        Self {
            ttl: 64,
            maximum_attempts: 3,
            backoff_base_ns: 100_000_000,
            jitter_ns: 10_000_000,
            recent_capacity: 1_024,
            signal_interval_ns: 100_000_000,
            mtu_bytes: 9_000,
        }
    }
}

/// Inputs that vary per lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupCase {
    /// Seed.
    pub seed: u64,
    /// Logical lookup ordinal.
    pub ordinal: u64,
    /// Source node.
    pub source: u32,
    /// Destination node.
    pub destination: u32,
    /// Required forwarding hops.
    pub required_hops: u32,
    /// Source coordinate depth.
    pub origin_depth: u32,
    /// Target coordinate depth.
    pub target_depth: u32,
    /// Extra candidates selected by Bloom false positives.
    pub false_positive_candidates: u32,
    /// Force every otherwise successful response onto a broken reverse path.
    pub reverse_path_broken: bool,
    /// Return a negative outcome even when TTL/MTU allow forwarding.
    pub target_absent: bool,
}

/// Stateful recent-request and signal-rate-limit model.
#[derive(Debug, Clone)]
pub struct LookupService {
    config: LookupConfig,
    recent: VecDeque<u64>,
    signal_last_sent: BTreeMap<RoutingSignal, u64>,
}

impl LookupService {
    /// Create a lookup service.
    pub fn new(config: LookupConfig) -> Self {
        Self {
            config,
            recent: VecDeque::new(),
            signal_last_sent: BTreeMap::new(),
        }
    }

    /// Execute a lookup including deterministic retries.
    pub fn execute(&mut self, case: &LookupCase, start_ns: u64) -> LookupRun {
        let mut counters = LookupCounters {
            lookups: 1,
            ..LookupCounters::default()
        };
        let mut attempts = Vec::new();
        let mut at = start_ns;
        let mut parent = None;
        let mut final_outcome = LookupOutcome::Negative;
        for attempt in 0..self.config.maximum_attempts {
            if attempt > 0 {
                counters.retries += 1;
                let exponent = 1_u64 << attempt.min(31);
                let jitter = if self.config.jitter_ns == 0 {
                    0
                } else {
                    stable_id(case.seed, case.ordinal, attempt, 99) % self.config.jitter_ns
                };
                at = at
                    .saturating_add(self.config.backoff_base_ns.saturating_mul(exponent))
                    .saturating_add(jitter);
            }
            let request_id = stable_id(case.seed, case.ordinal, attempt, 0);
            let causal_id = format!("lookup-{:016x}-attempt-{attempt}", case.ordinal);
            self.remember(request_id);
            let request_bytes = lookup_request_bytes(case.origin_depth);
            let response_bytes = lookup_response_bytes(case.target_depth);
            let deduplicated = case.false_positive_candidates / 2;
            let mut signal = None;
            let outcome = if case.required_hops > u32::from(self.config.ttl) {
                LookupOutcome::TtlExpired
            } else if request_bytes + ESTABLISHED_FMP_OVERHEAD_BYTES > self.config.mtu_bytes
                || response_bytes + ESTABLISHED_FMP_OVERHEAD_BYTES > self.config.mtu_bytes
            {
                signal = Some(RoutingSignal::MtuExceeded);
                LookupOutcome::MtuExceeded
            } else if case.target_absent {
                LookupOutcome::Negative
            } else if case.reverse_path_broken {
                signal = Some(RoutingSignal::PathBroken);
                LookupOutcome::ReversePathBroken
            } else {
                LookupOutcome::Success
            };
            counters.attempts += 1;
            counters.deduplicated += u64::from(deduplicated);
            counters.false_positive_candidates += u64::from(case.false_positive_candidates);
            counters.request_message_bytes =
                counters.request_message_bytes.saturating_add(request_bytes);
            if !matches!(
                outcome,
                LookupOutcome::TtlExpired | LookupOutcome::MtuExceeded
            ) {
                counters.response_message_bytes = counters
                    .response_message_bytes
                    .saturating_add(response_bytes);
            }
            if let Some(kind) = signal {
                if self.signal_allowed(kind, at) {
                    *counters.signals.entry(kind).or_default() += 1;
                } else {
                    counters.signals_rate_limited += 1;
                }
            }
            attempts.push(LookupAttempt {
                request_id,
                causal_id: causal_id.clone(),
                parent_causal_id: parent.clone(),
                attempt,
                started_at_ns: at,
                ttl: self.config.ttl,
                required_hops: case.required_hops,
                request_message_bytes: request_bytes,
                response_message_bytes: if matches!(
                    outcome,
                    LookupOutcome::TtlExpired | LookupOutcome::MtuExceeded
                ) {
                    0
                } else {
                    response_bytes
                },
                false_positive_candidates: case.false_positive_candidates,
                deduplicated,
                outcome,
                signal,
            });
            parent = Some(causal_id);
            final_outcome = outcome;
            if outcome == LookupOutcome::Success {
                counters.successes = 1;
                break;
            }
        }
        if counters.successes == 0 {
            counters.failures = 1;
        }
        counters.recovery_time_ns = attempts
            .last()
            .map_or(0, |attempt| attempt.started_at_ns.saturating_sub(start_ns));
        LookupRun {
            attempts,
            outcome: final_outcome,
            counters,
        }
    }

    /// Generate a CoordsRequired signal for a cache miss subject to the same limiter.
    pub fn coords_required(&mut self, at_ns: u64) -> bool {
        self.signal_allowed(RoutingSignal::CoordsRequired, at_ns)
    }

    fn remember(&mut self, request_id: u64) {
        if self.recent.contains(&request_id) {
            return;
        }
        self.recent.push_back(request_id);
        while self.recent.len() > self.config.recent_capacity {
            self.recent.pop_front();
        }
    }

    fn signal_allowed(&mut self, signal: RoutingSignal, at_ns: u64) -> bool {
        let allowed = self
            .signal_last_sent
            .get(&signal)
            .is_none_or(|last| at_ns.saturating_sub(*last) >= self.config.signal_interval_ns);
        if allowed {
            self.signal_last_sent.insert(signal, at_ns);
        }
        allowed
    }
}

/// Lookup configuration error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LookupError {
    /// Depth makes the codec length overflow.
    #[error("lookup coordinate depth {0} overflows the modeled wire length")]
    DepthOverflow(u32),
}

/// Exact production-encoder LookupRequest bytes at a depth.
pub fn lookup_request_bytes(depth: u32) -> u64 {
    LOOKUP_REQUEST_DEPTH_0_BYTES + 16 * u64::from(depth)
}

/// Exact production-encoder LookupResponse bytes at a depth.
pub fn lookup_response_bytes(depth: u32) -> u64 {
    LOOKUP_RESPONSE_DEPTH_0_BYTES + 16 * u64::from(depth)
}

/// Exact PathBroken FSP-framed bytes with optional last-known depth.
pub fn path_broken_bytes(depth: Option<u32>) -> u64 {
    40 + depth.map_or(0, |value| 16 * (u64::from(value) + 1))
}

fn stable_id(seed: u64, ordinal: u64, attempt: u32, lane: u64) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(attempt.to_le_bytes());
    hasher.update(lane.to_le_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[0..8].try_into().expect("slice length"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(hops: u32) -> LookupCase {
        LookupCase {
            seed: 5,
            ordinal: u64::from(hops),
            source: 0,
            destination: 1,
            required_hops: hops,
            origin_depth: hops,
            target_depth: hops,
            false_positive_candidates: 4,
            reverse_path_broken: false,
            target_absent: false,
        }
    }

    #[test]
    fn ttl_63_64_65_and_beyond_are_explicit() {
        for (hops, expected) in [
            (63, LookupOutcome::Success),
            (64, LookupOutcome::Success),
            (65, LookupOutcome::TtlExpired),
            (128, LookupOutcome::TtlExpired),
        ] {
            let mut service = LookupService::new(LookupConfig {
                mtu_bytes: 65_535,
                maximum_attempts: 1,
                ..LookupConfig::default()
            });
            assert_eq!(service.execute(&case(hops), 0).outcome, expected);
        }
    }

    #[test]
    fn executable_codec_sizes_and_mtu_boundary_match() {
        assert_eq!(lookup_request_bytes(0), 62);
        assert_eq!(lookup_response_bytes(0), 109);
        assert_eq!(lookup_request_bytes(64), 1_086);
        assert_eq!(lookup_response_bytes(64), 1_133);
        assert_eq!(path_broken_bytes(None), 40);
        assert_eq!(COORDS_REQUIRED_BYTES, 38);
        assert_eq!(MTU_EXCEEDED_BYTES, 40);
        let mut service = LookupService::new(LookupConfig {
            mtu_bytes: lookup_response_bytes(64) + ESTABLISHED_FMP_OVERHEAD_BYTES - 1,
            maximum_attempts: 1,
            ..LookupConfig::default()
        });
        assert_eq!(
            service.execute(&case(64), 0).outcome,
            LookupOutcome::MtuExceeded
        );
    }

    #[test]
    fn retry_lineage_dedup_jitter_and_signal_limits_are_deterministic() {
        let mut broken = case(5);
        broken.reverse_path_broken = true;
        let config = LookupConfig {
            signal_interval_ns: 1_000_000_000,
            ..LookupConfig::default()
        };
        let mut first = LookupService::new(config.clone());
        let mut second = LookupService::new(config);
        let left = first.execute(&broken, 0);
        let right = second.execute(&broken, 0);
        assert_eq!(left, right);
        assert_eq!(left.attempts.len(), 3);
        assert_eq!(left.counters.retries, 2);
        assert_eq!(
            left.attempts[1].parent_causal_id.as_deref(),
            Some(left.attempts[0].causal_id.as_str())
        );
        assert_eq!(left.counters.deduplicated, 6);
        assert!(left.counters.signals_rate_limited > 0);
    }
}
