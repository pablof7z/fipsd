//! Abstract media assignment policies and causal failover.

use crate::GeneratedTopology;
pub use fips_transport::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportAssignment {
    pub policy: TransportAssignmentPolicy,
    pub profiles: BTreeMap<String, MediaProfile>,
    pub edge_profiles: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailoverOutcome {
    pub causal_id: String,
    pub parent_causal_id: String,
    pub from_profile: String,
    pub to_profile: String,
    pub session_preserved: bool,
    pub reconnect_complete_ns: u64,
}

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
                        ids.iter().position(|id| id.contains("tor")).unwrap_or(0)
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

fn draw(seed: u64, ordinal: u64) -> usize {
    let mut value = seed ^ ordinal.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (value ^ (value >> 27)) as usize
}
