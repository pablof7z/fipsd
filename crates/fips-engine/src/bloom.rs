//! Deterministic Bloom representations and split-horizon replacement accounting.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Pinned FIPS v1 Bloom defaults.
pub const DEFAULT_BITS: usize = 8192;
/// Pinned FIPS v1 double-hash count.
pub const DEFAULT_HASH_COUNT: u8 = 5;
/// Executable-codec FilterAnnounce message bytes.
pub const FILTER_ANNOUNCE_BYTES: u64 = 1035;
/// Established FMP bytes for one FilterAnnounce.
pub const FILTER_ANNOUNCE_FMP_BYTES: u64 = 1071;

/// Bloom fidelity selected for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BloomMode {
    /// Packed production-equivalent bit vector.
    ExactBits,
    /// Sorted exact bit indices.
    SparseBits,
    /// Seeded statistical occupancy only.
    Occupancy,
}

/// A mode-specific Bloom filter with stable behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BloomModel {
    /// Packed exact bits.
    Exact {
        /// Bit storage.
        bits: Vec<u8>,
        /// Hash functions.
        hash_count: u8,
    },
    /// Sparse exact set-bit indices.
    Sparse {
        /// Set bits.
        bits: BTreeSet<usize>,
        /// Filter width.
        num_bits: usize,
        /// Hash functions.
        hash_count: u8,
        /// Maximum sparse population before callers should cross over.
        crossover_bits: usize,
    },
    /// Occupancy approximation with no per-item state.
    Occupancy {
        /// Filter width.
        num_bits: usize,
        /// Hash functions.
        hash_count: u8,
        /// Approximate set-bit count.
        occupied_bits: u64,
        /// Inserted element count.
        insertions: u64,
    },
}

impl BloomModel {
    /// Create a FIPS-v1-sized representation.
    pub fn new(mode: BloomMode) -> Self {
        Self::with_params(mode, DEFAULT_BITS, DEFAULT_HASH_COUNT)
    }

    /// Create a representation with explicit parameters.
    pub fn with_params(mode: BloomMode, num_bits: usize, hash_count: u8) -> Self {
        assert!(num_bits > 0 && num_bits % 8 == 0);
        assert!(hash_count > 0);
        match mode {
            BloomMode::ExactBits => Self::Exact {
                bits: vec![0; num_bits / 8],
                hash_count,
            },
            BloomMode::SparseBits => Self::Sparse {
                bits: BTreeSet::new(),
                num_bits,
                hash_count,
                crossover_bits: num_bits / 8,
            },
            BloomMode::Occupancy => Self::Occupancy {
                num_bits,
                hash_count,
                occupied_bits: 0,
                insertions: 0,
            },
        }
    }

    /// Active representation.
    pub fn mode(&self) -> BloomMode {
        match self {
            Self::Exact { .. } => BloomMode::ExactBits,
            Self::Sparse { .. } => BloomMode::SparseBits,
            Self::Occupancy { .. } => BloomMode::Occupancy,
        }
    }

    /// Insert one byte string and return modeled bitwise work.
    pub fn insert(&mut self, value: &[u8]) -> u64 {
        let indexes = indexes(value, self.num_bits(), self.hash_count());
        match self {
            Self::Exact { bits, .. } => {
                for index in indexes {
                    bits[index / 8] |= 1 << (index % 8);
                }
            }
            Self::Sparse { bits, .. } => {
                bits.extend(indexes);
            }
            Self::Occupancy {
                num_bits,
                hash_count,
                occupied_bits,
                insertions,
            } => {
                *insertions += 1;
                let empty_probability = (1.0 - 1.0 / *num_bits as f64)
                    .powi(i32::from(*hash_count) * i32::try_from(*insertions).unwrap_or(i32::MAX));
                *occupied_bits = ((*num_bits as f64) * (1.0 - empty_probability)).round() as u64;
            }
        }
        u64::from(self.hash_count())
    }

    /// Exact membership for exact/sparse modes and seeded occupancy draw otherwise.
    ///
    /// `known_present` preserves Bloom's no-false-negative contract when the
    /// occupancy representation is driven by an external population model.
    pub fn contains_seeded(
        &self,
        value: &[u8],
        seed: u64,
        query_ordinal: u64,
        known_present: bool,
    ) -> bool {
        if known_present {
            return true;
        }
        match self {
            Self::Exact { bits, .. } => indexes(value, self.num_bits(), self.hash_count())
                .into_iter()
                .all(|index| bits[index / 8] & (1 << (index % 8)) != 0),
            Self::Sparse { bits, .. } => indexes(value, self.num_bits(), self.hash_count())
                .into_iter()
                .all(|index| bits.contains(&index)),
            Self::Occupancy { .. } => seeded_unit(seed, query_ordinal, value) < self.fpr(),
        }
    }

    /// Union another compatible model into this one.
    pub fn union_assign(&mut self, other: &Self) -> Result<u64, BloomError> {
        if self.num_bits() != other.num_bits() || self.hash_count() != other.hash_count() {
            return Err(BloomError::Incompatible);
        }
        let work = (self.num_bits() / 8) as u64;
        match (self, other) {
            (Self::Exact { bits: left, .. }, Self::Exact { bits: right, .. }) => {
                for (left, right) in left.iter_mut().zip(right) {
                    *left |= right;
                }
            }
            (Self::Sparse { bits: left, .. }, Self::Sparse { bits: right, .. }) => {
                left.extend(right.iter().copied());
            }
            (
                Self::Occupancy {
                    num_bits,
                    occupied_bits,
                    insertions,
                    ..
                },
                Self::Occupancy {
                    occupied_bits: right,
                    insertions: right_insertions,
                    ..
                },
            ) => {
                let m = *num_bits as f64;
                let left_empty = 1.0 - *occupied_bits as f64 / m;
                let right_empty = 1.0 - *right as f64 / m;
                *occupied_bits = (m * (1.0 - left_empty * right_empty)).round() as u64;
                *insertions = insertions.saturating_add(*right_insertions);
            }
            _ => return Err(BloomError::ModeMismatch),
        }
        Ok(work)
    }

    /// Set-bit occupancy.
    pub fn occupied_bits(&self) -> u64 {
        match self {
            Self::Exact { bits, .. } => bits.iter().map(|byte| u64::from(byte.count_ones())).sum(),
            Self::Sparse { bits, .. } => bits.len() as u64,
            Self::Occupancy { occupied_bits, .. } => *occupied_bits,
        }
    }

    /// Fill ratio.
    pub fn fill_ratio(&self) -> f64 {
        self.occupied_bits() as f64 / self.num_bits() as f64
    }

    /// Current false-positive probability.
    pub fn fpr(&self) -> f64 {
        self.fill_ratio().powi(i32::from(self.hash_count()))
    }

    /// Swamidass-Baldi cardinality estimate, rejected above an antipoison cap.
    pub fn estimated_cardinality(&self, max_fpr: f64) -> Option<f64> {
        let occupied = self.occupied_bits() as f64;
        let width = self.num_bits() as f64;
        if occupied >= width || self.fpr() > max_fpr {
            return None;
        }
        Some(-(width / f64::from(self.hash_count())) * (1.0 - occupied / width).ln())
    }

    /// Whether sparse storage has reached its declared crossover.
    pub fn sparse_crossover_reached(&self) -> bool {
        matches!(self, Self::Sparse { bits, crossover_bits, .. } if bits.len() >= *crossover_bits)
    }

    /// Filter width.
    pub fn num_bits(&self) -> usize {
        match self {
            Self::Exact { bits, .. } => bits.len() * 8,
            Self::Sparse { num_bits, .. } | Self::Occupancy { num_bits, .. } => *num_bits,
        }
    }

    /// Hash count.
    pub fn hash_count(&self) -> u8 {
        match self {
            Self::Exact { hash_count, .. }
            | Self::Sparse { hash_count, .. }
            | Self::Occupancy { hash_count, .. } => *hash_count,
        }
    }
}

/// Peer role for split-horizon reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PeerRole {
    /// Current tree parent.
    Parent,
    /// Current tree child.
    Child,
    /// Non-tree peer.
    Mesh,
}

/// Exact replacement-wave accounting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BloomWaveCounters {
    /// Role-sensitive outgoing recomputations.
    pub recomputations: u64,
    /// Modeled bytewise OR/hash work.
    pub bitwise_operations: u64,
    /// Replacement requests.
    pub requested: u64,
    /// Requests folded into a pending replacement.
    pub coalesced: u64,
    /// Constructed full replacements.
    pub constructed: u64,
    /// Sent replacements.
    pub sent: u64,
    /// FPR/MTU rejections.
    pub rejected: u64,
    /// Exact serialized message bytes.
    pub message_bytes: u64,
    /// Exact established-FMP bytes.
    pub fmp_bytes: u64,
    /// Sends by destination role.
    pub by_role: BTreeMap<PeerRole, u64>,
}

/// One accepted full replacement entering the shared link service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BloomTransmission {
    /// Initiating semantic action.
    pub causal_id: String,
    /// Stable destination-peer key.
    pub peer: u32,
    /// Destination role.
    pub role: PeerRole,
    /// Virtual send time.
    pub at_ns: u64,
    /// Exact established-FMP bytes.
    pub frame_bytes: u64,
}

/// Deterministic per-peer replacement debounce.
#[derive(Debug, Clone)]
pub struct BloomReplacementWave {
    debounce_ns: u64,
    last_sent: BTreeMap<u32, u64>,
    pending: BTreeMap<u32, (u64, PeerRole, String)>,
    /// Reconciled counters.
    pub counters: BloomWaveCounters,
    /// Stable accepted transmission records.
    pub transmissions: Vec<BloomTransmission>,
}

impl BloomReplacementWave {
    /// Create the pinned 500 ms replacement scheduler.
    pub fn new(debounce_ns: u64) -> Self {
        Self {
            debounce_ns,
            last_sent: BTreeMap::new(),
            pending: BTreeMap::new(),
            counters: BloomWaveCounters::default(),
            transmissions: Vec::new(),
        }
    }

    /// Request a role-sensitive replacement. Returns its due time.
    pub fn request(&mut self, peer: u32, role: PeerRole, now_ns: u64) -> u64 {
        self.request_causal(peer, role, now_ns, format!("bloom:peer-{peer:08}"))
    }

    /// Request a replacement attributed to an initiating semantic action.
    pub fn request_causal(
        &mut self,
        peer: u32,
        role: PeerRole,
        now_ns: u64,
        causal_id: impl Into<String>,
    ) -> u64 {
        self.counters.requested += 1;
        self.counters.recomputations += 1;
        self.counters.bitwise_operations += (DEFAULT_BITS / 8) as u64;
        let due = self.last_sent.get(&peer).map_or(now_ns, |last| {
            now_ns.max(last.saturating_add(self.debounce_ns))
        });
        if self
            .pending
            .insert(peer, (due, role, causal_id.into()))
            .is_some()
        {
            self.counters.coalesced += 1;
        }
        due
    }

    /// Execute all replacements due at or before `now_ns`.
    pub fn flush(&mut self, now_ns: u64, max_fpr: f64, actual_fpr: f64, mtu: u64) {
        let due = self
            .pending
            .iter()
            .filter(|(_, (at, _, _))| *at <= now_ns)
            .map(|(peer, (at, role, causal_id))| (*peer, *at, *role, causal_id.clone()))
            .collect::<Vec<_>>();
        for (peer, at, role, causal_id) in due {
            self.pending.remove(&peer);
            self.counters.constructed += 1;
            if actual_fpr > max_fpr || FILTER_ANNOUNCE_FMP_BYTES > mtu {
                self.counters.rejected += 1;
                continue;
            }
            self.counters.sent += 1;
            self.counters.message_bytes += FILTER_ANNOUNCE_BYTES;
            self.counters.fmp_bytes += FILTER_ANNOUNCE_FMP_BYTES;
            *self.counters.by_role.entry(role).or_default() += 1;
            self.last_sent.insert(peer, at);
            self.transmissions.push(BloomTransmission {
                causal_id,
                peer,
                role,
                at_ns: at,
                frame_bytes: FILTER_ANNOUNCE_FMP_BYTES,
            });
        }
    }

    /// Pending replacement count.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Check exact request lifecycle reconciliation.
    pub fn reconciles(&self) -> bool {
        self.counters.requested
            == self.counters.coalesced + self.counters.constructed + self.pending.len() as u64
            && self.counters.constructed == self.counters.sent + self.counters.rejected
            && self.counters.message_bytes == self.counters.sent * FILTER_ANNOUNCE_BYTES
            && self.counters.fmp_bytes == self.counters.sent * FILTER_ANNOUNCE_FMP_BYTES
    }
}

/// Bloom modeling error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BloomError {
    /// Width or hash count differs.
    #[error("Bloom filters have incompatible parameters")]
    Incompatible,
    /// Exact, sparse, and occupancy representations cannot be directly unioned.
    #[error("Bloom representations use different modes")]
    ModeMismatch,
}

fn indexes(value: &[u8], num_bits: usize, hash_count: u8) -> Vec<usize> {
    let hash = Sha256::digest(value);
    let h1 = u64::from_le_bytes(hash[0..8].try_into().expect("slice length"));
    let h2 = u64::from_le_bytes(hash[8..16].try_into().expect("slice length"));
    (0..hash_count)
        .map(|ordinal| h1.wrapping_add(u64::from(ordinal).wrapping_mul(h2)) as usize % num_bits)
        .collect()
}

fn seeded_unit(seed: u64, ordinal: u64, value: &[u8]) -> f64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(value);
    let digest = hasher.finalize();
    let draw = u64::from_le_bytes(digest[0..8].try_into().expect("slice length"));
    draw as f64 / u64::MAX as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_sparse_agree_before_crossover() {
        let mut exact = BloomModel::new(BloomMode::ExactBits);
        let mut sparse = BloomModel::new(BloomMode::SparseBits);
        for value in 0_u64..100 {
            exact.insert(&value.to_le_bytes());
            sparse.insert(&value.to_le_bytes());
        }
        assert!(!sparse.sparse_crossover_reached());
        assert_eq!(exact.occupied_bits(), sparse.occupied_bits());
        assert_eq!(exact.fpr(), sparse.fpr());
        for value in 0_u64..200 {
            assert_eq!(
                exact.contains_seeded(&value.to_le_bytes(), 7, value, value < 100),
                sparse.contains_seeded(&value.to_le_bytes(), 7, value, value < 100)
            );
        }
    }

    #[test]
    fn occupancy_tracks_analytical_expectation_and_seeded_fpr() {
        let mut occupancy = BloomModel::new(BloomMode::Occupancy);
        for value in 0_u64..1000 {
            occupancy.insert(&value.to_le_bytes());
        }
        let expected_fill = 1.0 - (-5.0_f64 * 1000.0 / 8192.0).exp();
        assert!((occupancy.fill_ratio() - expected_fill).abs() < 0.001);
        let positives = (0_u64..20_000)
            .filter(|ordinal| occupancy.contains_seeded(b"absent", 99, *ordinal, false))
            .count() as f64;
        let observed = positives / 20_000.0;
        assert!((observed - occupancy.fpr()).abs() < 0.01);
        assert_eq!(
            occupancy.contains_seeded(b"same", 99, 4, false),
            occupancy.contains_seeded(b"same", 99, 4, false)
        );
    }

    #[test]
    fn split_horizon_boundaries_and_directional_waves_reconcile() {
        let mut wave = BloomReplacementWave::new(500_000_000);
        assert_eq!(wave.request(1, PeerRole::Parent, 0), 0);
        wave.flush(0, 0.20, 0.01, 1500);
        assert_eq!(wave.request(1, PeerRole::Parent, 499_000_000), 500_000_000);
        assert_eq!(wave.request(1, PeerRole::Parent, 500_000_000), 500_000_000);
        assert_eq!(wave.request(2, PeerRole::Child, 501_000_000), 501_000_000);
        wave.flush(501_000_000, 0.20, 0.01, 1500);
        assert_eq!(wave.counters.coalesced, 1);
        assert_eq!(wave.counters.sent, 3);
        assert_eq!(wave.counters.by_role[&PeerRole::Parent], 2);
        assert_eq!(wave.counters.by_role[&PeerRole::Child], 1);
        assert!(wave.reconciles());
    }

    #[test]
    fn antipoison_and_mtu_rejections_are_counted() {
        let mut wave = BloomReplacementWave::new(500_000_000);
        wave.request(1, PeerRole::Mesh, 0);
        wave.flush(0, 0.20, 0.21, 1500);
        wave.request(2, PeerRole::Mesh, 1);
        wave.flush(1, 0.20, 0.01, 1000);
        assert_eq!(wave.counters.rejected, 2);
        assert!(wave.reconciles());
    }
}
