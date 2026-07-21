//! Property-based topology and event-sequence generation with composable shrinking.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use thiserror::Error;

/// Requested connectivity class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectivityClass {
    /// Every node is reachable.
    Connected,
    /// At least two components are present.
    Disconnected,
}

/// Property-generator constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorConstraints {
    /// Stable node count represented without per-node allocation at large scale.
    pub nodes: u64,
    /// Connectivity requirement.
    pub connectivity: ConnectivityClass,
    /// Maximum undirected degree for materialized samples.
    pub maximum_degree: u32,
    /// Event count.
    pub event_count: usize,
    /// Minimum event separation.
    pub minimum_interval_ns: u64,
}

/// Materialized or symbolic generated topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedTopology {
    /// Total represented nodes.
    pub represented_nodes: u64,
    /// Individually materialized sample nodes.
    pub sampled_nodes: u32,
    /// Sample edges with stable IDs.
    pub edges: Vec<(u32, u32)>,
    /// Requested class.
    pub connectivity: ConnectivityClass,
    /// Whether the large population is symbolic.
    pub symbolic_population: bool,
}

/// Valid protocol event family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedEventKind {
    Join,
    Leave,
    Flap,
    Partition,
    Merge,
    CostWave,
    TimerRace,
}

/// One generated event retained in failure evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedEvent {
    pub id: String,
    pub kind: GeneratedEventKind,
    pub at_ns: u64,
    pub target: u32,
    pub causal_parent: Option<String>,
}

/// Replayable property-generated input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedInput {
    pub seed: u64,
    pub constraints: GeneratorConstraints,
    pub topology: GeneratedTopology,
    pub events: Vec<GeneratedEvent>,
}

impl GeneratedInput {
    /// Deterministic generator-level shrink candidates for hierarchical shrinking.
    pub fn shrink_candidates(&self) -> Vec<Self> {
        let mut candidates = Vec::new();
        if self.events.len() > 1 {
            let mut events = self.clone();
            events.events.truncate(self.events.len().div_ceil(2));
            events.constraints.event_count = events.events.len();
            candidates.push(events);
        }
        if self.topology.represented_nodes > 2 {
            let mut nodes = self.clone();
            nodes.topology.represented_nodes = self.topology.represented_nodes.div_ceil(2).max(2);
            nodes.constraints.nodes = nodes.topology.represented_nodes;
            let sampled = nodes.topology.represented_nodes.min(u64::from(u32::MAX)) as u32;
            nodes.topology.sampled_nodes = nodes.topology.sampled_nodes.min(sampled);
            nodes.topology.edges.retain(|(a, b)| {
                *a < nodes.topology.sampled_nodes && *b < nodes.topology.sampled_nodes
            });
            candidates.push(nodes);
        }
        if self.events.iter().any(|event| event.at_ns > 1) {
            let mut timing = self.clone();
            for event in &mut timing.events {
                event.at_ns /= 2;
            }
            timing.constraints.minimum_interval_ns /= 2;
            candidates.push(timing);
        }
        candidates
    }
}

/// Property generator failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GeneratorError {
    #[error("property generator requires at least two nodes, got {0}")]
    TooFewNodes(u64),
    #[error("maximum degree {maximum_degree} cannot satisfy requested connectivity")]
    Degree { maximum_degree: u32 },
    #[error("generated topology violated declared connectivity {0:?}")]
    Connectivity(ConnectivityClass),
}

/// Generate a valid topology and event sequence from a seed.
pub fn generate_input(
    seed: u64,
    constraints: GeneratorConstraints,
) -> Result<GeneratedInput, GeneratorError> {
    if constraints.nodes < 2 {
        return Err(GeneratorError::TooFewNodes(constraints.nodes));
    }
    if constraints.maximum_degree == 0
        || (constraints.connectivity == ConnectivityClass::Connected
            && constraints.nodes > 2
            && constraints.maximum_degree < 2)
    {
        return Err(GeneratorError::Degree {
            maximum_degree: constraints.maximum_degree,
        });
    }
    let sampled_nodes = constraints.nodes.min(256) as u32;
    let symbolic_population = constraints.nodes > u64::from(sampled_nodes);
    let split = sampled_nodes.div_ceil(2);
    let mut edges = Vec::new();
    for node in 1..sampled_nodes {
        if constraints.connectivity == ConnectivityClass::Disconnected && node == split {
            continue;
        }
        edges.push((node - 1, node));
    }
    let maximum_edges = (u64::from(sampled_nodes) * u64::from(constraints.maximum_degree) / 2)
        .min(u64::from(sampled_nodes) * u64::from(sampled_nodes.saturating_sub(1)) / 2);
    let mut ordinal = 0_u64;
    let target_edges = maximum_edges.min(u64::from(sampled_nodes) * 2);
    let attempt_limit = u64::from(sampled_nodes)
        .saturating_mul(u64::from(sampled_nodes))
        .saturating_mul(16);
    while (edges.len() as u64) < target_edges && ordinal < attempt_limit {
        let left = (draw(seed, ordinal, 0) % u64::from(sampled_nodes)) as u32;
        let right = (draw(seed, ordinal, 1) % u64::from(sampled_nodes)) as u32;
        ordinal += 1;
        if left == right || edges.contains(&(left.min(right), left.max(right))) {
            continue;
        }
        if constraints.connectivity == ConnectivityClass::Disconnected
            && (left < split) != (right < split)
        {
            continue;
        }
        if degree(&edges, left) >= constraints.maximum_degree
            || degree(&edges, right) >= constraints.maximum_degree
        {
            continue;
        }
        edges.push((left.min(right), left.max(right)));
    }
    edges.sort_unstable();
    let topology = GeneratedTopology {
        represented_nodes: constraints.nodes,
        sampled_nodes,
        edges,
        connectivity: constraints.connectivity,
        symbolic_population,
    };
    if connected(&topology) != (constraints.connectivity == ConnectivityClass::Connected) {
        return Err(GeneratorError::Connectivity(constraints.connectivity));
    }
    let kinds = [
        GeneratedEventKind::Join,
        GeneratedEventKind::Leave,
        GeneratedEventKind::Flap,
        GeneratedEventKind::Partition,
        GeneratedEventKind::Merge,
        GeneratedEventKind::CostWave,
        GeneratedEventKind::TimerRace,
    ];
    let events = (0..constraints.event_count)
        .map(|index| GeneratedEvent {
            id: format!("generated-{index:08}"),
            kind: kinds[index % kinds.len()],
            at_ns: (index as u64 + 1).saturating_mul(constraints.minimum_interval_ns),
            target: (draw(seed, index as u64, 2) % u64::from(sampled_nodes)) as u32,
            causal_parent: index
                .checked_sub(1)
                .map(|parent| format!("generated-{parent:08}")),
        })
        .collect();
    Ok(GeneratedInput {
        seed,
        constraints,
        topology,
        events,
    })
}

fn degree(edges: &[(u32, u32)], node: u32) -> u32 {
    edges
        .iter()
        .filter(|(a, b)| *a == node || *b == node)
        .count() as u32
}

fn connected(topology: &GeneratedTopology) -> bool {
    let mut seen = BTreeSet::from([0_u32]);
    let mut queue = VecDeque::from([0_u32]);
    while let Some(node) = queue.pop_front() {
        for &(a, b) in &topology.edges {
            let next = if a == node {
                Some(b)
            } else if b == node {
                Some(a)
            } else {
                None
            };
            if next.is_some_and(|next| seen.insert(next)) {
                queue.push_back(next.expect("checked Some"));
            }
        }
    }
    seen.len() == topology.sampled_nodes as usize
}

fn draw(seed: u64, ordinal: u64, lane: u64) -> u64 {
    let mut value = seed ^ ordinal.wrapping_mul(0xD6E8_FEB8_6659_FD93) ^ lane.rotate_left(29);
    value ^= value >> 32;
    value = value.wrapping_mul(0xA24B_AED4_963E_E407);
    value ^ (value >> 28)
}
