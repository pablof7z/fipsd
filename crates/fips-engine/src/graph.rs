//! Compact stable-ID graph storage and deterministic topology generation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};
use std::mem::size_of;
use thiserror::Error;

/// Stable node identifier used as an array index.
pub type NodeId = u32;
/// Stable edge identifier used as an array index.
pub type EdgeId = u32;

/// FIPS 128-bit node address, ordered lexicographically like the production type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct NodeAddress(pub [u8; 16]);

impl NodeAddress {
    /// Deterministic high-range address for an initial benign node.
    pub fn initial(id: NodeId) -> Self {
        let mut bytes = [0_u8; 16];
        bytes[0] = 0x80;
        bytes[12..].copy_from_slice(&id.to_be_bytes());
        Self(bytes)
    }

    /// Address exactly one integer lower, if the address is non-zero.
    pub fn one_lower(self) -> Option<Self> {
        let mut bytes = self.0;
        for byte in bytes.iter_mut().rev() {
            if *byte > 0 {
                *byte -= 1;
                return Some(Self(bytes));
            }
            *byte = u8::MAX;
        }
        None
    }

    /// Lower-case fixed-width hexadecimal representation.
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    /// Parse a fixed-width hexadecimal address.
    pub fn from_hex(value: &str) -> Result<Self, GraphError> {
        let bytes = hex::decode(value).map_err(|_| GraphError::Address(value.to_owned()))?;
        let bytes: [u8; 16] = bytes
            .try_into()
            .map_err(|_| GraphError::Address(value.to_owned()))?;
        Ok(Self(bytes))
    }
}

/// Supported exact-graph topology families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TopologyKind {
    /// Caller-provided undirected edges.
    Explicit,
    /// Linear chain.
    Chain,
    /// Binary breadth-first tree.
    BalancedTree,
    /// Seed-permuted circulant regular graph.
    RandomRegular,
    /// Seeded preferential-attachment graph.
    ScaleFree,
}

impl TopologyKind {
    /// Parse the Campaign spelling.
    pub fn parse(value: &str) -> Result<Self, GraphError> {
        match value {
            "explicit" => Ok(Self::Explicit),
            "chain" => Ok(Self::Chain),
            "balanced-tree" => Ok(Self::BalancedTree),
            "random-regular" => Ok(Self::RandomRegular),
            "scale-free" => Ok(Self::ScaleFree),
            other => Err(GraphError::UnsupportedTopology(other.to_owned())),
        }
    }
}

/// Deterministic attachment selectors for adversarial arrivals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentSelector {
    /// Node currently advertising the minimum root.
    CurrentRoot,
    /// Smallest-ID active leaf.
    Leaf,
    /// Highest-degree active node, tie-broken by ID.
    Hub,
    /// Smallest-ID articulation point.
    Articulation,
    /// Seeded active node selection.
    Random,
}

impl AttachmentSelector {
    /// Parse Campaign spellings and their original M0 aliases.
    pub fn parse(value: &str) -> Result<Self, GraphError> {
        match value {
            "current-root" => Ok(Self::CurrentRoot),
            "leaf" | "random-leaf" => Ok(Self::Leaf),
            "hub" | "highest-degree" => Ok(Self::Hub),
            "articulation" | "articulation-point" => Ok(Self::Articulation),
            "random" => Ok(Self::Random),
            other => Err(GraphError::UnsupportedSelector(other.to_owned())),
        }
    }
}

/// Estimated compact storage footprint, excluding optional inspection output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphMemoryFootprint {
    /// Bytes of fixed-width node columns per node.
    pub fixed_bytes_per_node: usize,
    /// Bytes of fixed-width edge columns per edge.
    pub fixed_bytes_per_edge: usize,
    /// Total allocated bytes estimated from vector capacities.
    pub allocated_bytes: usize,
}

/// Structure-of-arrays node and edge storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStore {
    addresses: Vec<NodeAddress>,
    active: Vec<bool>,
    roots: Vec<NodeId>,
    parents: Vec<Option<NodeId>>,
    sequences: Vec<u64>,
    ancestries: Vec<Vec<NodeId>>,
    resource_classes: Vec<u16>,
    edge_a: Vec<NodeId>,
    edge_b: Vec<NodeId>,
}

impl GraphStore {
    /// Create a store with `node_count` stable IDs and no edges.
    pub fn with_nodes(node_count: u32) -> Self {
        let addresses = (0..node_count)
            .map(NodeAddress::initial)
            .collect::<Vec<_>>();
        let active = vec![true; node_count as usize];
        let roots = (0..node_count).collect::<Vec<_>>();
        let parents = vec![None; node_count as usize];
        let sequences = vec![1; node_count as usize];
        let ancestries = (0..node_count).map(|id| vec![id]).collect::<Vec<_>>();
        let resource_classes = vec![0; node_count as usize];
        Self {
            addresses,
            active,
            roots,
            parents,
            sequences,
            ancestries,
            resource_classes,
            edge_a: Vec::new(),
            edge_b: Vec::new(),
        }
    }

    /// Generate a deterministic topology.
    pub fn generate(
        kind: TopologyKind,
        node_count: u32,
        average_degree: u32,
        seed: u64,
        explicit_edges: &[(NodeId, NodeId)],
    ) -> Result<Self, GraphError> {
        if node_count == 0 {
            return Err(GraphError::EmptyGraph);
        }
        let mut graph = Self::with_nodes(node_count);
        match kind {
            TopologyKind::Explicit => {
                for &(a, b) in explicit_edges {
                    graph.add_edge(a, b)?;
                }
            }
            TopologyKind::Chain => {
                for node in 1..node_count {
                    graph.add_edge(node - 1, node)?;
                }
            }
            TopologyKind::BalancedTree => {
                for node in 1..node_count {
                    graph.add_edge((node - 1) / 2, node)?;
                }
            }
            TopologyKind::RandomRegular => {
                graph.generate_regular(average_degree, seed)?;
            }
            TopologyKind::ScaleFree => {
                graph.generate_scale_free(average_degree.max(2) / 2, seed)?;
            }
        }
        graph.validate()?;
        if node_count > 1 && !graph.is_connected_active() {
            return Err(GraphError::Disconnected);
        }
        Ok(graph)
    }

    /// Number of stable node slots.
    pub fn node_count(&self) -> usize {
        self.addresses.len()
    }

    /// Number of stable undirected edges.
    pub fn edge_count(&self) -> usize {
        self.edge_a.len()
    }

    /// Iterate stable node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        0..self.node_count() as NodeId
    }

    /// Return an address by stable node ID.
    pub fn address(&self, id: NodeId) -> Result<NodeAddress, GraphError> {
        self.addresses
            .get(id as usize)
            .copied()
            .ok_or(GraphError::DanglingNode(id))
    }

    /// Replace an address while preserving the stable ID.
    pub fn set_address(&mut self, id: NodeId, address: NodeAddress) -> Result<(), GraphError> {
        let slot = self
            .addresses
            .get_mut(id as usize)
            .ok_or(GraphError::DanglingNode(id))?;
        *slot = address;
        Ok(())
    }

    /// Whether a node participates in the current graph.
    pub fn is_active(&self, id: NodeId) -> bool {
        self.active.get(id as usize).copied().unwrap_or(false)
    }

    /// Activate or reserve a stable node slot.
    pub fn set_active(&mut self, id: NodeId, active: bool) -> Result<(), GraphError> {
        let slot = self
            .active
            .get_mut(id as usize)
            .ok_or(GraphError::DanglingNode(id))?;
        *slot = active;
        Ok(())
    }

    /// Mark the highest stable IDs inactive for later arrivals.
    pub fn reserve_arrivals(&mut self, count: u32) -> Result<(), GraphError> {
        if count as usize >= self.node_count() {
            return Err(GraphError::ArrivalCount(count));
        }
        let start = self.node_count() - count as usize;
        for id in start..self.node_count() {
            self.active[id] = false;
        }
        Ok(())
    }

    /// Add a stable undirected edge.
    pub fn add_edge(&mut self, a: NodeId, b: NodeId) -> Result<EdgeId, GraphError> {
        if a as usize >= self.node_count() {
            return Err(GraphError::DanglingNode(a));
        }
        if b as usize >= self.node_count() {
            return Err(GraphError::DanglingNode(b));
        }
        if a == b {
            return Err(GraphError::SelfEdge(a));
        }
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        if self
            .edge_a
            .iter()
            .zip(&self.edge_b)
            .any(|(&left, &right)| left == a && right == b)
        {
            return Err(GraphError::DuplicateEdge(a, b));
        }
        let id = self.edge_count() as EdgeId;
        self.edge_a.push(a);
        self.edge_b.push(b);
        Ok(id)
    }

    /// Edge endpoints by stable ID.
    pub fn edge(&self, id: EdgeId) -> Result<(NodeId, NodeId), GraphError> {
        let index = id as usize;
        match (self.edge_a.get(index), self.edge_b.get(index)) {
            (Some(&a), Some(&b)) => Ok((a, b)),
            _ => Err(GraphError::DanglingEdge(id)),
        }
    }

    /// Stable edge ID between two nodes.
    pub fn edge_between(&self, left: NodeId, right: NodeId) -> Option<EdgeId> {
        let (a, b) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        self.edge_a
            .iter()
            .zip(&self.edge_b)
            .position(|(&x, &y)| x == a && y == b)
            .map(|index| index as EdgeId)
    }

    /// Active neighbors in stable ID order.
    pub fn active_neighbors(&self, id: NodeId) -> Vec<NodeId> {
        let mut neighbors = self
            .edge_a
            .iter()
            .zip(&self.edge_b)
            .filter_map(|(&a, &b)| {
                if a == id && self.is_active(b) {
                    Some(b)
                } else if b == id && self.is_active(a) {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        neighbors.sort_unstable();
        neighbors
    }

    /// Degree among active nodes.
    pub fn active_degree(&self, id: NodeId) -> usize {
        self.active_neighbors(id).len()
    }

    /// Read tree state columns.
    pub fn root(&self, id: NodeId) -> NodeId {
        self.roots[id as usize]
    }

    /// Read selected parent.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.parents[id as usize]
    }

    /// Read current ancestry `[self, ..., root]`.
    pub fn ancestry(&self, id: NodeId) -> &[NodeId] {
        &self.ancestries[id as usize]
    }

    /// Read current sequence.
    pub fn sequence(&self, id: NodeId) -> u64 {
        self.sequences[id as usize]
    }

    /// Atomically update tree state and increment the declaration sequence.
    pub fn set_tree(
        &mut self,
        id: NodeId,
        parent: Option<NodeId>,
        ancestry: Vec<NodeId>,
    ) -> Result<(), GraphError> {
        if ancestry.first() != Some(&id) {
            return Err(GraphError::InvalidAncestry(id));
        }
        let unique = ancestry.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != ancestry.len()
            || ancestry
                .iter()
                .any(|node| *node as usize >= self.node_count())
        {
            return Err(GraphError::InvalidAncestry(id));
        }
        if parent != ancestry.get(1).copied() {
            return Err(GraphError::InvalidAncestry(id));
        }
        let root = *ancestry.last().ok_or(GraphError::InvalidAncestry(id))?;
        self.parents[id as usize] = parent;
        self.roots[id as usize] = root;
        self.ancestries[id as usize] = ancestry;
        self.sequences[id as usize] = self.sequences[id as usize].saturating_add(1);
        Ok(())
    }

    /// Reset a node to a self-root state without changing its address.
    pub fn reset_self_root(&mut self, id: NodeId) -> Result<(), GraphError> {
        self.set_tree(id, None, vec![id])
    }

    /// Resolve an attachment selector or return an explicit diagnostic.
    pub fn select_attachment(
        &self,
        selector: AttachmentSelector,
        seed: u64,
        ordinal: u64,
    ) -> Result<NodeId, GraphError> {
        let active = self
            .node_ids()
            .filter(|id| self.is_active(*id))
            .collect::<Vec<_>>();
        if active.is_empty() {
            return Err(GraphError::NoAttachment(selector));
        }
        let selected = match selector {
            AttachmentSelector::CurrentRoot => active
                .iter()
                .copied()
                .min_by_key(|id| (self.address(self.root(*id)).ok(), *id)),
            AttachmentSelector::Leaf => active
                .iter()
                .copied()
                .filter(|id| self.active_degree(*id) <= 1)
                .min(),
            AttachmentSelector::Hub => active
                .iter()
                .copied()
                .max_by_key(|id| (self.active_degree(*id), std::cmp::Reverse(*id))),
            AttachmentSelector::Articulation => self.articulation_points().into_iter().next(),
            AttachmentSelector::Random => {
                let index = deterministic_u64(seed, ordinal) as usize % active.len();
                Some(active[index])
            }
        };
        selected.ok_or(GraphError::NoAttachment(selector))
    }

    /// Active articulation points in stable ID order.
    pub fn articulation_points(&self) -> Vec<NodeId> {
        let active_count = self.node_ids().filter(|id| self.is_active(*id)).count();
        self.node_ids()
            .filter(|candidate| {
                self.is_active(*candidate)
                    && active_count > 2
                    && self.reachable_count_excluding(Some(*candidate)) + 1 < active_count
            })
            .collect()
    }

    /// True if all active nodes share one component.
    pub fn is_connected_active(&self) -> bool {
        let active_count = self.node_ids().filter(|id| self.is_active(*id)).count();
        active_count <= 1 || self.reachable_count_excluding(None) == active_count
    }

    /// Validate stable IDs, edge uniqueness, and tree columns.
    pub fn validate(&self) -> Result<(), GraphError> {
        let count = self.node_count();
        if self.active.len() != count
            || self.roots.len() != count
            || self.parents.len() != count
            || self.sequences.len() != count
            || self.ancestries.len() != count
            || self.resource_classes.len() != count
        {
            return Err(GraphError::ColumnLength);
        }
        if self.edge_a.len() != self.edge_b.len() {
            return Err(GraphError::ColumnLength);
        }
        let mut edges = BTreeSet::new();
        for (&a, &b) in self.edge_a.iter().zip(&self.edge_b) {
            if a as usize >= count || b as usize >= count {
                return Err(GraphError::DanglingEdge(edges.len() as EdgeId));
            }
            if !edges.insert((a, b)) {
                return Err(GraphError::DuplicateEdge(a, b));
            }
        }
        Ok(())
    }

    /// Canonical graph SHA-256 for golden generator fixtures.
    pub fn graph_sha256(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("graph serialization cannot fail");
        hex::encode(Sha256::digest(bytes))
    }

    /// Estimate fixed-width and currently allocated graph storage.
    pub fn memory_footprint(&self) -> GraphMemoryFootprint {
        let fixed_bytes_per_node = size_of::<NodeAddress>()
            + size_of::<bool>()
            + size_of::<NodeId>()
            + size_of::<Option<NodeId>>()
            + size_of::<u64>()
            + size_of::<u16>();
        let fixed_bytes_per_edge = size_of::<NodeId>() * 2;
        let allocated_bytes = self.addresses.capacity() * size_of::<NodeAddress>()
            + self.active.capacity() * size_of::<bool>()
            + self.roots.capacity() * size_of::<NodeId>()
            + self.parents.capacity() * size_of::<Option<NodeId>>()
            + self.sequences.capacity() * size_of::<u64>()
            + self.resource_classes.capacity() * size_of::<u16>()
            + self.edge_a.capacity() * size_of::<NodeId>()
            + self.edge_b.capacity() * size_of::<NodeId>()
            + self
                .ancestries
                .iter()
                .map(|path| path.capacity() * size_of::<NodeId>())
                .sum::<usize>();
        GraphMemoryFootprint {
            fixed_bytes_per_node,
            fixed_bytes_per_edge,
            allocated_bytes,
        }
    }

    fn reachable_count_excluding(&self, excluded: Option<NodeId>) -> usize {
        let Some(start) = self
            .node_ids()
            .find(|id| self.is_active(*id) && Some(*id) != excluded)
        else {
            return 0;
        };
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(node) = queue.pop_front() {
            for neighbor in self.active_neighbors(node) {
                if Some(neighbor) != excluded && seen.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        seen.len()
    }
}

/// Compact graph failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Zero-node graph.
    #[error("topology must contain at least one node")]
    EmptyGraph,
    /// Unknown topology spelling.
    #[error("unsupported topology generator: {0}")]
    UnsupportedTopology(String),
    /// Unknown attachment selector.
    #[error("unsupported attachment selector: {0}")]
    UnsupportedSelector(String),
    /// Node ID is outside the stable array.
    #[error("dangling node id {0}")]
    DanglingNode(NodeId),
    /// Edge ID is outside the stable array.
    #[error("dangling edge id {0}")]
    DanglingEdge(EdgeId),
    /// Self edges are invalid.
    #[error("self edge for node {0}")]
    SelfEdge(NodeId),
    /// Duplicate undirected edge.
    #[error("duplicate edge {0}-{1}")]
    DuplicateEdge(NodeId, NodeId),
    /// Structure-of-arrays columns drifted.
    #[error("graph storage columns have inconsistent lengths")]
    ColumnLength,
    /// Active graph is not connected.
    #[error("topology is disconnected")]
    Disconnected,
    /// Invalid regular degree request.
    #[error("cannot construct a {degree}-regular graph with {nodes} nodes")]
    RegularDegree {
        /// Node count.
        nodes: u32,
        /// Requested degree.
        degree: u32,
    },
    /// Arrival reservation would leave no initial node.
    #[error("arrival count {0} leaves no initial node")]
    ArrivalCount(u32),
    /// Selected topology has no matching active attachment.
    #[error("attachment selector {0:?} has no eligible active node")]
    NoAttachment(AttachmentSelector),
    /// Tree ancestry is malformed or cyclic.
    #[error("invalid ancestry for node {0}")]
    InvalidAncestry(NodeId),
    /// Address is not exactly 128 bits of hexadecimal.
    #[error("invalid 128-bit node address: {0}")]
    Address(String),
}

fn deterministic_u64(seed: u64, ordinal: u64) -> u64 {
    let mut state = seed ^ ordinal.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut output = state;
    output = (output ^ (output >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    output = (output ^ (output >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    output ^ (output >> 31)
}

#[path = "graph_generators.rs"]
mod generators;

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
