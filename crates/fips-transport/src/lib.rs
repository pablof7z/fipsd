//! Shared versioned transport profiles used by campaign generation and engines.

use serde::{Deserialize, Serialize};

/// Transport/media family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKind {
    Udp,
    Tcp,
    Ethernet,
    Wifi,
    Ble,
    Tor,
    Nym,
}

impl MediaKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "udp" => Some(Self::Udp),
            "tcp" => Some(Self::Tcp),
            "ethernet" => Some(Self::Ethernet),
            "wifi" | "wi-fi" => Some(Self::Wifi),
            "ble" | "bluetooth" => Some(Self::Ble),
            "tor" => Some(Self::Tor),
            "nym" => Some(Self::Nym),
            _ => None,
        }
    }
}

/// Stream/datagram behavior visible to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaOrdering {
    Datagram,
    Stream,
}

/// Whether profile values are measured or deliberately abstract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileProvenance {
    Abstract,
    Calibrated,
}

/// Versioned behavior profile with explicit effective MTU and overhead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaProfile {
    pub id: String,
    pub version: String,
    pub kind: MediaKind,
    pub provenance: ProfileProvenance,
    pub effective_mtu_bytes: u64,
    pub transport_overhead_bytes: u64,
    pub ordering: MediaOrdering,
    pub reliable: bool,
    pub latency_ns: u64,
    pub jitter_ns: u64,
    pub bandwidth_bps: u64,
    pub reconnect_ns: u64,
    pub head_of_line_blocking: bool,
}

/// Built-in abstract profiles. These values are not measured wire truth.
pub fn builtin_profiles() -> Vec<MediaProfile> {
    [
        profile(
            MediaKind::Udp,
            1472,
            28,
            MediaOrdering::Datagram,
            false,
            1,
            false,
        ),
        profile(
            MediaKind::Tcp,
            1460,
            40,
            MediaOrdering::Stream,
            true,
            1,
            true,
        ),
        profile(
            MediaKind::Ethernet,
            1500,
            18,
            MediaOrdering::Datagram,
            false,
            1,
            false,
        ),
        profile(
            MediaKind::Wifi,
            1472,
            32,
            MediaOrdering::Datagram,
            false,
            8,
            false,
        ),
        profile(
            MediaKind::Ble,
            244,
            12,
            MediaOrdering::Stream,
            true,
            20,
            true,
        ),
        profile(
            MediaKind::Tor,
            1280,
            64,
            MediaOrdering::Stream,
            true,
            200,
            true,
        ),
        profile(
            MediaKind::Nym,
            1200,
            96,
            MediaOrdering::Stream,
            true,
            400,
            true,
        ),
    ]
    .into_iter()
    .collect()
}

pub fn builtin_profile(kind: MediaKind) -> MediaProfile {
    builtin_profiles()
        .into_iter()
        .find(|profile| profile.kind == kind)
        .expect("every MediaKind has one built-in profile")
}

/// Seed-stable weighted selection. Zero-weight entries are never selected.
pub fn deterministic_weighted_index(seed: u64, ordinal: u64, weights: &[u64]) -> Option<usize> {
    let total = weights
        .iter()
        .try_fold(0_u64, |sum, weight| sum.checked_add(*weight))?;
    if total == 0 {
        return None;
    }
    let target = draw(seed, ordinal) % total;
    let mut cumulative = 0_u64;
    weights.iter().position(|weight| {
        cumulative = cumulative.saturating_add(*weight);
        target < cumulative
    })
}

fn profile(
    kind: MediaKind,
    mtu: u64,
    overhead: u64,
    ordering: MediaOrdering,
    reliable: bool,
    latency_ms: u64,
    hol: bool,
) -> MediaProfile {
    let name = format!("{kind:?}").to_lowercase();
    MediaProfile {
        id: format!("abstract-{name}-v1"),
        version: "1".to_owned(),
        kind,
        provenance: ProfileProvenance::Abstract,
        effective_mtu_bytes: mtu,
        transport_overhead_bytes: overhead,
        ordering,
        reliable,
        latency_ns: latency_ms * 1_000_000,
        jitter_ns: latency_ms * 250_000,
        bandwidth_bps: if matches!(kind, MediaKind::Ble) {
            1_000_000
        } else {
            100_000_000
        },
        reconnect_ns: latency_ms * 5_000_000,
        head_of_line_blocking: hol,
    }
}

fn draw(seed: u64, ordinal: u64) -> u64 {
    let mut value = seed ^ ordinal.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^ (value >> 27)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_assignments_are_seed_stable_and_respect_zeroes() {
        let left = (0..100)
            .map(|node| deterministic_weighted_index(7, node, &[3, 0, 1]))
            .collect::<Vec<_>>();
        let right = (0..100)
            .map(|node| deterministic_weighted_index(7, node, &[3, 0, 1]))
            .collect::<Vec<_>>();
        assert_eq!(left, right);
        assert!(left.into_iter().flatten().all(|index| index != 1));
    }
}
