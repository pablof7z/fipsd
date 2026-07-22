use crate::document::{Representation, SourceRange};
use fips_artifact::RunArtifact;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkNode {
    pub id: u64,
    pub observed_events: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEdge {
    pub from: u64,
    pub to: u64,
    pub frames: u64,
    pub frame_bytes: u64,
    pub maximum_depth: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkView {
    pub mode: String,
    pub exact_nodes: Vec<NetworkNode>,
    pub exact_edges: Vec<NetworkEdge>,
    pub depth_frame_distribution: BTreeMap<u64, u64>,
    pub top_edges: Vec<NetworkEdge>,
    pub source: SourceRange,
    pub fidelity: String,
}

pub fn summarize_network(artifact: &RunArtifact, representation: Representation) -> NetworkView {
    let mut nodes = BTreeMap::<u64, u64>::new();
    let mut edges = BTreeMap::<(u64, u64), NetworkEdge>::new();
    let mut depths = BTreeMap::<u64, u64>::new();
    for event in &artifact.event_trace {
        let Some(from) = event.data.get("from").and_then(|value| value.as_u64()) else {
            continue;
        };
        let Some(to) = event.data.get("to").and_then(|value| value.as_u64()) else {
            continue;
        };
        *nodes.entry(from).or_default() += 1;
        *nodes.entry(to).or_default() += 1;
        let depth = event
            .data
            .get("depth")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let bytes = event
            .data
            .get("frame_bytes")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        *depths.entry(depth).or_default() += 1;
        let edge = edges.entry((from, to)).or_insert(NetworkEdge {
            from,
            to,
            frames: 0,
            frame_bytes: 0,
            maximum_depth: 0,
        });
        edge.frames += 1;
        edge.frame_bytes = edge.frame_bytes.saturating_add(bytes);
        edge.maximum_depth = edge.maximum_depth.max(depth);
    }
    let mut top_edges = edges.values().cloned().collect::<Vec<_>>();
    top_edges.sort_by(|left, right| {
        right
            .frame_bytes
            .cmp(&left.frame_bytes)
            .then_with(|| (left.from, left.to).cmp(&(right.from, right.to)))
    });
    top_edges.truncate(20);
    let exact = representation == Representation::ExactGraph;
    NetworkView {
        mode: if exact { "exact" } else { "aggregated" }.to_owned(),
        exact_nodes: if exact {
            nodes
                .into_iter()
                .map(|(id, observed_events)| NetworkNode { id, observed_events })
                .collect()
        } else {
            Vec::new()
        },
        exact_edges: if exact {
            edges.into_values().collect()
        } else {
            Vec::new()
        },
        depth_frame_distribution: depths,
        top_edges,
        source: SourceRange {
            collection: "event_trace/tree-announce".to_owned(),
            start: 0,
            end_exclusive: artifact.event_trace.len(),
            total: artifact.event_trace.len(),
        },
        fidelity: if exact {
            "all observed event edges; physical peer edges remain distinct from root/parent semantics"
        } else {
            "deterministic depth distribution and top-20 observed edge sample"
        }
        .to_owned(),
    }
}
