//! Deterministic coordinate-cache capacity, freshness, and invalidation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable coordinate cache key.
pub type CacheKey = [u8; 16];

/// One cached coordinate path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinateEntry {
    /// Destination address.
    pub destination: CacheKey,
    /// Root generation under which the path was learned.
    pub root: CacheKey,
    /// Stable node-ID path.
    pub path: Vec<u32>,
    /// Executable-codec coordinate bytes.
    pub coordinate_bytes: u64,
    /// Absolute expiry.
    pub expires_at_ns: u64,
    /// Monotonic deterministic LRU touch.
    pub last_touch: u64,
}

/// Cache counters used by recovery reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheCounters {
    /// Successful insert or refresh operations.
    pub insertions: u64,
    /// Successful fresh reads.
    pub hits: u64,
    /// Absent or expired reads.
    pub misses: u64,
    /// Expired entries removed on access/sweep.
    pub expirations: u64,
    /// Capacity evictions.
    pub evictions: u64,
    /// Entries invalidated by topology change.
    pub invalidations: u64,
    /// Coordinate bytes learned during warmup.
    pub warmup_bytes: u64,
    /// Maximum simultaneously active entries.
    pub peak_entries: u64,
    /// Maximum coordinate memory.
    pub peak_bytes: u64,
}

/// Why a topology change invalidates cached coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalidation {
    /// One destination disappeared.
    Node(CacheKey),
    /// Global root generation changed.
    Root(CacheKey),
    /// A local ancestor was replaced; invalidate only paths containing it.
    PathNode(u32),
}

/// Bounded deterministic coordinate cache.
#[derive(Debug, Clone)]
pub struct CoordinateCache {
    capacity: usize,
    ttl_ns: u64,
    touch: u64,
    entries: BTreeMap<CacheKey, CoordinateEntry>,
    /// Reconciled cache metrics.
    pub counters: CacheCounters,
}

impl CoordinateCache {
    /// Create a cache with explicit entry capacity and TTL.
    pub fn new(capacity: usize, ttl_ns: u64) -> Self {
        Self {
            capacity,
            ttl_ns,
            touch: 0,
            entries: BTreeMap::new(),
            counters: CacheCounters::default(),
        }
    }

    /// Insert or refresh a path, evicting the least-recently-touched key.
    pub fn insert(&mut self, destination: CacheKey, root: CacheKey, path: Vec<u32>, now_ns: u64) {
        self.expire(now_ns);
        self.touch = self.touch.saturating_add(1);
        let coordinate_bytes = 2 + 16 * path.len() as u64;
        if !self.entries.contains_key(&destination) && self.entries.len() >= self.capacity {
            if let Some(victim) = self
                .entries
                .values()
                .min_by_key(|entry| (entry.last_touch, entry.destination))
                .map(|entry| entry.destination)
            {
                self.entries.remove(&victim);
                self.counters.evictions += 1;
            }
        }
        if self.capacity > 0 {
            self.entries.insert(
                destination,
                CoordinateEntry {
                    destination,
                    root,
                    path,
                    coordinate_bytes,
                    expires_at_ns: now_ns.saturating_add(self.ttl_ns),
                    last_touch: self.touch,
                },
            );
            self.counters.warmup_bytes =
                self.counters.warmup_bytes.saturating_add(coordinate_bytes);
            self.counters.insertions = self.counters.insertions.saturating_add(1);
            self.record_peaks();
        }
    }

    /// Get a fresh entry and refresh both LRU position and expiry.
    pub fn get(&mut self, destination: &CacheKey, now_ns: u64) -> Option<&CoordinateEntry> {
        let expired = self
            .entries
            .get(destination)
            .is_some_and(|entry| entry.expires_at_ns <= now_ns);
        if expired {
            self.entries.remove(destination);
            self.counters.expirations += 1;
        }
        if let Some(entry) = self.entries.get_mut(destination) {
            self.touch = self.touch.saturating_add(1);
            entry.last_touch = self.touch;
            entry.expires_at_ns = now_ns.saturating_add(self.ttl_ns);
            self.counters.hits += 1;
            self.entries.get(destination)
        } else {
            self.counters.misses += 1;
            None
        }
    }

    /// Apply a precisely scoped invalidation and return removed entries.
    pub fn invalidate(&mut self, cause: &Invalidation) -> u64 {
        let keys = self
            .entries
            .values()
            .filter(|entry| match cause {
                Invalidation::Node(destination) => entry.destination == *destination,
                Invalidation::Root(root) => entry.root == *root,
                Invalidation::PathNode(node) => entry.path.contains(node),
            })
            .map(|entry| entry.destination)
            .collect::<Vec<_>>();
        for key in &keys {
            self.entries.remove(key);
        }
        self.counters.invalidations = self
            .counters
            .invalidations
            .saturating_add(keys.len() as u64);
        keys.len() as u64
    }

    /// Remove all expired entries.
    pub fn expire(&mut self, now_ns: u64) -> u64 {
        let keys = self
            .entries
            .values()
            .filter(|entry| entry.expires_at_ns <= now_ns)
            .map(|entry| entry.destination)
            .collect::<Vec<_>>();
        for key in &keys {
            self.entries.remove(key);
        }
        self.counters.expirations = self.counters.expirations.saturating_add(keys.len() as u64);
        keys.len() as u64
    }

    /// Active entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entries are active.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Current coordinate bytes.
    pub fn memory_bytes(&self) -> u64 {
        self.entries
            .values()
            .map(|entry| entry.coordinate_bytes)
            .sum()
    }

    /// Fresh entry count at a point in virtual time.
    pub fn fresh_entries(&self, now_ns: u64) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.expires_at_ns > now_ns)
            .count()
    }

    fn record_peaks(&mut self) {
        self.counters.peak_entries = self.counters.peak_entries.max(self.entries.len() as u64);
        self.counters.peak_bytes = self.counters.peak_bytes.max(self.memory_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: u8) -> CacheKey {
        [value; 16]
    }

    #[test]
    fn root_and_local_path_invalidations_have_distinct_scope() {
        let mut cache = CoordinateCache::new(8, 1_000);
        cache.insert(key(1), key(9), vec![1, 2, 3], 0);
        cache.insert(key(2), key(9), vec![4, 5], 0);
        cache.insert(key(3), key(8), vec![1, 6], 0);
        assert_eq!(cache.invalidate(&Invalidation::PathNode(1)), 2);
        assert_eq!(cache.len(), 1);
        cache.insert(key(1), key(9), vec![1, 2, 3], 1);
        assert_eq!(cache.invalidate(&Invalidation::Root(key(9))), 2);
        assert!(cache.is_empty());
        assert_eq!(cache.counters.invalidations, 4);
    }

    #[test]
    fn touch_expiry_and_capacity_thrash_are_visible() {
        let mut cache = CoordinateCache::new(2, 10);
        cache.insert(key(1), key(9), vec![1], 0);
        cache.insert(key(2), key(9), vec![2, 3], 0);
        assert!(cache.get(&key(1), 5).is_some());
        cache.insert(key(3), key(9), vec![3], 6);
        assert!(cache.get(&key(2), 6).is_none());
        assert_eq!(cache.counters.evictions, 1);
        assert!(cache.get(&key(1), 14).is_some());
        assert!(cache.get(&key(1), 25).is_none());
        assert_eq!(cache.counters.expirations, 1);
        assert_eq!(cache.counters.hits, 2);
        assert_eq!(cache.counters.misses, 2);
        assert!(cache.counters.peak_bytes > 0);
    }
}
