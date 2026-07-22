//! Versioned abstract media profiles, assignment policies, and causal failover.

use crate::GeneratedTopology;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Transport/media family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKind {
    Udp,
    Tcp,
    Ethernet,
    Ble,
    Tor,
    Nym,
}

/// Stream/datagram behavior visible to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaOrdering {
    Datagram,
    Stream,
}

/// Whether values are measured or deliberately abstract.
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

/// Supported deterministic assignment policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportAssignmentPolicy {
    Homogeneous,
    RandomMixed,
    DepthCorrelated,
    HighLatencySpine,
    DescendingMtuSpine,
    Failover,
}

/// Effective profile for each sampled edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportAssignment {
    pub policy: TransportAssignmentPolicy,
    pub profiles: BTreeMap<String, MediaProfile>,
    pub edge_profiles: BTreeMap<String, String>,
}

/// Session outcome caused by a profile transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailoverOutcome {
    pub causal_id: String,
    pub parent_causal_id: String,
    pub from_profile: String,
    pub to_profile: String,
    pub session_preserved: bool,
    pub reconnect_complete_ns: u64,
}

/// Built-in abstract profiles; values are not presented as measured wire truth.
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

/// Assign profiles to topology edges with seed-stable provenance.
pub fn assign_transports(
    topology: &GeneratedTopology,
    policy: TransportAssignmentPolicy,
    seed: u64,
) -> TransportAssignment {
    let profiles = builtin_profiles()
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<BTreeMap<_, _>>();
    let ids = profiles.keys().cloned().collect::<Vec<_>>();
    let edge_profiles = topology
        .edges
        .iter()
        .enumerate()
        .map(|(index, &(a, b))| {
            let selected = match policy {
                TransportAssignmentPolicy::Homogeneous => 0,
                TransportAssignmentPolicy::RandomMixed => draw(seed, index as u64) % ids.len(),
                TransportAssignmentPolicy::DepthCorrelated => (a.max(b) as usize) % ids.len(),
                TransportAssignmentPolicy::HighLatencySpine => {
                    if a == 0 || b == 0 {
                        4
                    } else {
                        0
                    }
                }
                TransportAssignmentPolicy::DescendingMtuSpine => {
                    (a.max(b) as usize).min(ids.len() - 1)
                }
                TransportAssignmentPolicy::Failover => index % 2,
            };
            (format!("edge:{a}-{b}"), ids[selected].clone())
        })
        .collect();
    TransportAssignment {
        policy,
        profiles,
        edge_profiles,
    }
}

/// Apply one causal failover without hiding the session consequence.
pub fn failover(
    assignment: &mut TransportAssignment,
    edge: &str,
    to_profile: &str,
    parent: &str,
    at_ns: u64,
) -> Option<FailoverOutcome> {
    let from_profile = assignment.edge_profiles.get(edge)?.clone();
    let from = assignment.profiles.get(&from_profile)?;
    let to = assignment.profiles.get(to_profile)?;
    assignment
        .edge_profiles
        .insert(edge.to_owned(), to_profile.to_owned());
    Some(FailoverOutcome {
        causal_id: format!("failover:{edge}:{at_ns}"),
        parent_causal_id: parent.to_owned(),
        from_profile,
        to_profile: to_profile.to_owned(),
        session_preserved: from.ordering == to.ordering && from.reliable == to.reliable,
        reconnect_complete_ns: at_ns.saturating_add(to.reconnect_ns),
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
    let name = format!("{:?}", kind).to_lowercase();
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

fn draw(seed: u64, ordinal: u64) -> usize {
    let mut value = seed ^ ordinal.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (value ^ (value >> 27)) as usize
}
