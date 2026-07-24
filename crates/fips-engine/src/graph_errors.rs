use super::*;
use thiserror::Error;

/// Compact graph failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphError {
    #[error("topology must contain at least one node")]
    EmptyGraph,
    #[error("unsupported topology generator: {0}")]
    UnsupportedTopology(String),
    #[error("unsupported attachment selector: {0}")]
    UnsupportedSelector(String),
    #[error("dangling node id {0}")]
    DanglingNode(NodeId),
    #[error("dangling edge id {0}")]
    DanglingEdge(EdgeId),
    #[error("self edge for node {0}")]
    SelfEdge(NodeId),
    #[error("duplicate edge {0}-{1}")]
    DuplicateEdge(NodeId, NodeId),
    #[error("graph storage columns have inconsistent lengths")]
    ColumnLength,
    #[error("topology is disconnected")]
    Disconnected,
    #[error("cannot construct a {degree}-regular graph with {nodes} nodes")]
    RegularDegree { nodes: u32, degree: u32 },
    #[error("arrival count {0} leaves no initial node")]
    ArrivalCount(u32),
    #[error("attachment selector {0:?} has no eligible active node")]
    NoAttachment(AttachmentSelector),
    #[error("invalid ancestry for node {0}")]
    InvalidAncestry(NodeId),
    #[error("invalid 128-bit node address: {0}")]
    Address(String),
}
