//! Seed-stable synthetic sessions and useful-payload traffic.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Executable-codec depth-zero session setup message 1.
pub const SESSION_SETUP_MESSAGE_BYTES: u64 = 76;
/// Executable-codec depth-zero session acknowledgement.
pub const SESSION_ACK_MESSAGE_BYTES: u64 = 100;

/// Supported synthetic traffic matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrafficModel {
    /// No useful traffic.
    Idle,
    /// Seeded source/destination pairs.
    UniformRandom,
    /// One-to-one permutation.
    Permutation,
    /// Every ordered pair.
    AllToAll,
    /// Skewed destination popularity.
    Zipf,
    /// Many sources to one destination.
    Incast,
    /// One source to many destinations.
    Outcast,
    /// Mixed large and small payloads.
    ElephantsAndMice,
    /// Long-lived sessions emitted as ordered payload segments.
    PersistentStreams,
    /// Explicit application-object transfers between authored endpoints.
    ExplicitTransfers,
    /// Seeded flows emitted in synchronized bursts.
    Bursty,
    /// Cross a deterministic bisection.
    CrossCut,
    /// Repeated session setup and teardown.
    SessionChurn,
    /// Cycle payload sizes around MTU boundaries.
    PayloadSweep,
}

impl TrafficModel {
    /// Parse Campaign spelling.
    pub fn parse(value: &str) -> Result<Self, TrafficError> {
        match value {
            "idle" => Ok(Self::Idle),
            "uniform-random" => Ok(Self::UniformRandom),
            "permutation" => Ok(Self::Permutation),
            "all-to-all" => Ok(Self::AllToAll),
            "zipf" => Ok(Self::Zipf),
            "incast" => Ok(Self::Incast),
            "outcast" => Ok(Self::Outcast),
            "elephants-and-mice" => Ok(Self::ElephantsAndMice),
            "persistent-streams" => Ok(Self::PersistentStreams),
            "explicit-transfers" => Ok(Self::ExplicitTransfers),
            "bursty" => Ok(Self::Bursty),
            "cross-min-cut" | "cross-cut" => Ok(Self::CrossCut),
            "session-churn" => Ok(Self::SessionChurn),
            "payload-sweep" => Ok(Self::PayloadSweep),
            other => Err(TrafficError::Unknown(other.to_owned())),
        }
    }
}

/// Session lifecycle action attached to a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionAction {
    /// Establish a session before delivery.
    Setup,
    /// Existing session carries data.
    Reuse,
    /// Rekey hook executes before delivery.
    Rekey,
    /// Tear down after this packet.
    Teardown,
}

/// Position of one payload offer inside a larger traffic process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FlowShape {
    /// Independent one-shot flow.
    Single,
    /// One segment of a persistent stream.
    StreamSegment {
        /// Stable stream identifier.
        stream_id: String,
        /// Zero-based segment index.
        segment_index: u32,
        /// Total segments in the stream.
        segment_count: u32,
    },
    /// One visible byte range of an explicit application-object transfer.
    ApplicationTransfer {
        /// Stable transfer identifier.
        transfer_id: String,
        /// Zero-based visualization chunk index.
        chunk_index: u32,
        /// Total visualization chunks in the transfer.
        chunk_count: u32,
        /// Total application bytes in the transfer.
        total_bytes: u64,
        /// Inclusive byte offset of this chunk.
        byte_start: u64,
        /// Exclusive byte offset of this chunk.
        byte_end: u64,
    },
    /// One member of a synchronized burst.
    BurstMember {
        /// Zero-based burst index.
        burst_index: u64,
        /// Zero-based member index.
        member_index: u32,
        /// Number of offers in this burst.
        member_count: u32,
    },
}

/// One offered useful-payload flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
    /// Stable flow ID.
    pub id: String,
    /// Source node.
    pub source: u32,
    /// Destination node.
    pub destination: u32,
    /// Offer time.
    pub offered_at_ns: u64,
    /// Application payload only.
    pub useful_payload_bytes: u64,
    /// Session lifecycle action.
    pub session_action: SessionAction,
    /// Stream/burst lineage.
    pub shape: FlowShape,
}

/// One explicitly authored application-object transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSpec {
    /// Stable transfer identifier.
    pub id: String,
    /// Source node.
    pub source: u32,
    /// Destination node.
    pub destination: u32,
    /// Complete useful application byte count.
    pub total_bytes: u64,
    /// Bytes represented by one visible progress chunk.
    pub visualization_chunk_bytes: u64,
    /// Virtual time at which the transfer begins.
    pub start_ns: u64,
}

/// Generator inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficConfig {
    /// Matrix.
    pub model: TrafficModel,
    /// Node count.
    pub nodes: u32,
    /// Number of offered flows; ignored for all-to-all.
    pub flow_count: u64,
    /// Base payload.
    pub payload_bytes: u64,
    /// Aggregate useful-payload offer rate.
    pub rate_bps: u64,
    /// Offer interval.
    pub interval_ns: u64,
    /// Segments emitted by each persistent stream.
    pub segments_per_stream: u32,
    /// Simultaneous offers in each burst.
    pub burst_size: u32,
    /// Virtual-time gap between burst starts.
    pub burst_interval_ns: u64,
    /// Seed.
    pub seed: u64,
    /// Explicit application-object transfers.
    pub transfers: Vec<TransferSpec>,
}

/// Generated session/traffic accounting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficPlan {
    /// Stable offered flows.
    pub flows: Vec<Flow>,
    /// Application payload offered.
    pub offered_useful_bytes: u64,
    /// Session setup operations.
    pub session_setups: u64,
    /// Session teardown operations.
    pub session_teardowns: u64,
    /// Abstract encryption/rekey hooks.
    pub rekeys: u64,
    /// Exact setup/ack message bytes, excluding transport/FMP overhead.
    pub setup_message_bytes: u64,
}

impl TrafficPlan {
    /// Generate a deterministic traffic plan and validate offered load.
    pub fn generate(config: &TrafficConfig) -> Result<Self, TrafficError> {
        generation::generate(config)
    }
}

/// Traffic configuration error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrafficError {
    /// Unknown Campaign value.
    #[error("unknown traffic model {0}")]
    Unknown(String),
    /// Non-idle traffic requires at least two nodes.
    #[error("traffic requires at least two nodes, got {0}")]
    TooFewNodes(u32),
    /// Useful payload cannot be zero.
    #[error("non-idle traffic requires a positive payload")]
    ZeroPayload,
    /// An explicit transfer endpoint does not exist.
    #[error("transfer {id} endpoint {node} is outside the {nodes}-node topology")]
    InvalidTransferEndpoint {
        /// Transfer.
        id: String,
        /// Invalid node.
        node: u32,
        /// Topology size.
        nodes: u32,
    },
    /// Persistent streams require at least one segment.
    #[error("persistent streams require segments_per_stream greater than zero")]
    ZeroSegments,
    /// Bursty traffic requires at least one member per burst.
    #[error("bursty traffic requires burst_size greater than zero")]
    ZeroBurstSize,
    /// Bursty traffic requires time to advance between bursts.
    #[error("bursty traffic requires burst_interval_ns greater than zero")]
    ZeroBurstInterval,
    /// The generated flow count overflowed.
    #[error("traffic flow count overflow")]
    FlowCountOverflow,
    /// Generator produced an invalid self-flow.
    #[error("traffic flow {ordinal} has identical source/destination {node}")]
    SelfFlow {
        /// Flow ordinal.
        ordinal: u64,
        /// Node.
        node: u32,
    },
    /// Aggregate does not equal flows.
    #[error("offered load drift: recorded {recorded}, projected {projected}")]
    OfferedLoadDrift {
        /// Recorded aggregate.
        recorded: u64,
        /// Per-flow projection.
        projected: u64,
    },
}

#[path = "traffic_generation.rs"]
mod generation;

#[cfg(test)]
#[path = "traffic_tests.rs"]
mod tests;
