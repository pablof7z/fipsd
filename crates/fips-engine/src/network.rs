//! Deterministic link serialization, loss, ordering, MTU, and queue service.

use crate::{EdgeId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;

const NANOS_PER_SECOND: u64 = 1_000_000_000;

#[path = "network_stream.rs"]
mod stream;
pub use stream::{StreamEnqueueRequest, StreamEnqueueResult};

/// Stream or datagram delivery semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkOrdering {
    /// Preserve send order at delivery.
    Stream,
    /// Permit deterministic latency jitter to reorder datagrams.
    Datagram,
}

/// Shared capacity class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkClass {
    /// Protocol control traffic.
    Control,
    /// Useful application payload.
    UsefulPayload,
}

/// Per-edge link configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkConfig {
    /// One-way propagation latency.
    pub latency_ns: u64,
    /// Maximum deterministic datagram jitter, exclusive.
    pub jitter_ns: u64,
    /// Shared serialization bandwidth.
    pub bandwidth_bps: u64,
    /// Independent deterministic loss probability in parts per million.
    pub loss_ppm: u32,
    /// Deterministic duplicate probability in parts per million.
    pub duplication_ppm: u32,
    /// Ordering contract.
    pub ordering: LinkOrdering,
    /// Effective link MTU including modeled transport/network overhead.
    pub mtu_bytes: u64,
    /// Maximum queued wire bytes per direction.
    pub queue_bytes: u64,
    /// Bytes added below the FMP frame by the selected transport profile.
    pub transport_overhead_bytes: u64,
}

impl Default for LinkConfig {
    fn default() -> Self {
        Self {
            latency_ns: 1_000_000,
            jitter_ns: 500_000,
            bandwidth_bps: 1_000_000_000,
            loss_ppm: 0,
            duplication_ppm: 0,
            ordering: LinkOrdering::Datagram,
            mtu_bytes: 9_000,
            queue_bytes: 1_048_576,
            transport_overhead_bytes: 28,
        }
    }
}

/// One scheduled link delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    /// Stable edge.
    pub edge_id: EdgeId,
    /// Sender.
    pub from: NodeId,
    /// Receiver.
    pub to: NodeId,
    /// Shared capacity class.
    pub class: LinkClass,
    /// Original FMP frame bytes, excluding modeled lower-layer overhead.
    pub frame_bytes: u64,
    /// Total bytes placed on the configured transport.
    pub wire_bytes: u64,
    /// Delivery virtual time.
    pub deliver_at_ns: u64,
    /// Zero for original, positive for deterministic duplicate copies.
    pub copy_ordinal: u8,
}

/// One logical frame offered to a directed link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnqueueRequest {
    /// Stable edge.
    pub edge_id: EdgeId,
    /// Sender.
    pub from: NodeId,
    /// Receiver.
    pub to: NodeId,
    /// Shared capacity class.
    pub class: LinkClass,
    /// FMP frame bytes before lower-layer overhead.
    pub frame_bytes: u64,
    /// Useful payload contained in the frame, if any.
    pub useful_payload_bytes: u64,
    /// Injected enqueue time.
    pub now_ns: u64,
}

/// Result of one logical enqueue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnqueueResult {
    /// Non-lost copies to schedule for delivery.
    pub deliveries: Vec<Delivery>,
    /// Total wire bytes serialized, including lost duplicates.
    pub transmitted_bytes: u64,
    /// Total bytes deterministically lost after transmission.
    pub lost_bytes: u64,
    /// Queue occupancy immediately after enqueue.
    pub queue_occupancy_bytes: u64,
}

/// Reconciled per-edge-direction counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkCounters {
    /// Logical frames accepted into the bounded queue.
    pub accepted_frames: u64,
    /// Logical frames rejected before serialization.
    pub rejected_frames: u64,
    /// Bytes serialized onto the link, including duplicates and lost copies.
    pub transmitted_bytes: u64,
    /// Serialized bytes lost before delivery.
    pub lost_bytes: u64,
    /// Bytes delivered to the receiver.
    pub delivered_bytes: u64,
    /// Bytes rejected by MTU or queue policy.
    pub rejected_bytes: u64,
    /// Useful payload bytes delivered, excluding all framing.
    pub useful_payload_bytes: u64,
    /// Peak queued transport bytes.
    pub peak_queue_bytes: u64,
}

#[derive(Debug, Clone, Default)]
struct DirectionState {
    last_stream_delivery_ns: u64,
    frame_sequence: u64,
    counters: LinkCounters,
}

#[derive(Debug, Clone, Default)]
struct CapacityState {
    next_serialization_ns: u64,
    queued: VecDeque<(u64, u64)>,
}

/// Stable link service for every directed edge.
#[derive(Debug, Clone)]
pub struct LinkService {
    seed: u64,
    configs: Vec<LinkConfig>,
    shared_groups: Vec<Option<u32>>,
    directions: BTreeMap<(EdgeId, NodeId, NodeId), DirectionState>,
    capacities: BTreeMap<(u64, NodeId, NodeId), CapacityState>,
}

impl LinkService {
    /// Create one identical configuration per undirected edge.
    pub fn uniform(seed: u64, edge_count: usize, config: LinkConfig) -> Self {
        Self {
            seed,
            configs: vec![config; edge_count],
            shared_groups: vec![None; edge_count],
            directions: BTreeMap::new(),
            capacities: BTreeMap::new(),
        }
    }

    /// Replace a particular edge's configuration.
    pub fn set_config(&mut self, edge: EdgeId, config: LinkConfig) -> Result<(), LinkError> {
        let index = edge as usize;
        if index == self.configs.len() {
            self.configs.push(config);
            self.shared_groups.push(None);
            return Ok(());
        }
        let slot = self
            .configs
            .get_mut(index)
            .ok_or(LinkError::UnknownEdge(edge))?;
        *slot = config;
        Ok(())
    }

    /// Put an edge on one half-duplex shared serialization and queue domain.
    pub fn set_shared_group(&mut self, edge: EdgeId, group: u32) -> Result<(), LinkError> {
        let slot = self
            .shared_groups
            .get_mut(edge as usize)
            .ok_or(LinkError::UnknownEdge(edge))?;
        *slot = Some(group);
        Ok(())
    }

    /// Read a particular edge's configuration.
    pub fn config(&self, edge: EdgeId) -> Result<&LinkConfig, LinkError> {
        self.configs
            .get(edge as usize)
            .ok_or(LinkError::UnknownEdge(edge))
    }

    /// Shared-medium group for an edge, if its two endpoints occupy one zone.
    pub fn shared_group(&self, edge: EdgeId) -> Option<u32> {
        self.shared_groups.get(edge as usize).copied().flatten()
    }

    /// Enqueue one logical frame and deterministically decide copies and loss.
    pub fn enqueue(&mut self, request: EnqueueRequest) -> Result<EnqueueResult, LinkError> {
        let EnqueueRequest {
            edge_id,
            from,
            to,
            class,
            frame_bytes,
            useful_payload_bytes,
            now_ns,
        } = request;
        let config = self.config(edge_id)?.clone();
        if config.bandwidth_bps == 0 {
            return Err(LinkError::ZeroBandwidth(edge_id));
        }
        let wire_bytes = frame_bytes
            .checked_add(config.transport_overhead_bytes)
            .ok_or(LinkError::Arithmetic)?;
        let capacity_key = match self.shared_groups[edge_id as usize] {
            Some(group) => (u64::from(group) | (1_u64 << 63), NodeId::MAX, NodeId::MAX),
            None => (u64::from(edge_id), from, to),
        };
        let capacity = self.capacities.entry(capacity_key).or_default();
        while capacity
            .queued
            .front()
            .is_some_and(|(complete, _)| *complete <= now_ns)
        {
            capacity.queued.pop_front();
        }
        let state = self.directions.entry((edge_id, from, to)).or_default();

        if wire_bytes > config.mtu_bytes {
            state.counters.rejected_frames += 1;
            state.counters.rejected_bytes += wire_bytes;
            return Err(LinkError::MtuExceeded {
                edge: edge_id,
                frame_bytes: wire_bytes,
                mtu_bytes: config.mtu_bytes,
            });
        }

        let duplicate = draw_ppm(self.seed, edge_id, from, to, state.frame_sequence, 0xD0)
            < config.duplication_ppm;
        let copies = if duplicate { 2_u64 } else { 1_u64 };
        let accepted_bytes = wire_bytes
            .checked_mul(copies)
            .ok_or(LinkError::Arithmetic)?;
        let queued_bytes = capacity.queued.iter().map(|(_, bytes)| bytes).sum::<u64>();
        if queued_bytes.saturating_add(accepted_bytes) > config.queue_bytes {
            state.counters.rejected_frames += 1;
            state.counters.rejected_bytes += accepted_bytes;
            return Err(LinkError::QueueFull {
                edge: edge_id,
                queued_bytes,
                frame_bytes: accepted_bytes,
                limit_bytes: config.queue_bytes,
            });
        }

        state.counters.accepted_frames += 1;
        let mut deliveries = Vec::new();
        let mut transmitted_bytes = 0_u64;
        let mut lost_bytes = 0_u64;
        for copy in 0..copies {
            let serialization_ns = serialization_delay_ns(wire_bytes, config.bandwidth_bps)?;
            let start_ns = capacity.next_serialization_ns.max(now_ns);
            let complete_ns = start_ns
                .checked_add(serialization_ns)
                .ok_or(LinkError::Arithmetic)?;
            capacity.next_serialization_ns = complete_ns;
            capacity.queued.push_back((complete_ns, wire_bytes));
            transmitted_bytes = transmitted_bytes
                .checked_add(wire_bytes)
                .ok_or(LinkError::Arithmetic)?;
            let lost = draw_ppm(
                self.seed,
                edge_id,
                from,
                to,
                state.frame_sequence,
                0x10 + copy as u8,
            ) < config.loss_ppm;
            if lost {
                lost_bytes += wire_bytes;
                continue;
            }
            let jitter_ns = if config.ordering == LinkOrdering::Datagram && config.jitter_ns > 0 {
                deterministic_u64(
                    self.seed ^ u64::from(edge_id),
                    state.frame_sequence ^ (copy << 32),
                ) % config.jitter_ns
            } else {
                0
            };
            let mut deliver_at_ns = complete_ns
                .checked_add(config.latency_ns)
                .and_then(|time| time.checked_add(jitter_ns))
                .ok_or(LinkError::Arithmetic)?;
            if config.ordering == LinkOrdering::Stream {
                deliver_at_ns = deliver_at_ns.max(state.last_stream_delivery_ns);
                state.last_stream_delivery_ns = deliver_at_ns;
            }
            deliveries.push(Delivery {
                edge_id,
                from,
                to,
                class,
                frame_bytes,
                wire_bytes,
                deliver_at_ns,
                copy_ordinal: copy as u8,
            });
        }
        state.frame_sequence += 1;
        state.counters.transmitted_bytes += transmitted_bytes;
        state.counters.lost_bytes += lost_bytes;
        let occupancy = queued_bytes + accepted_bytes;
        state.counters.peak_queue_bytes = state.counters.peak_queue_bytes.max(occupancy);
        if class == LinkClass::UsefulPayload && !deliveries.is_empty() {
            // Credited on actual delivery; retained here only to validate input.
            let _ = useful_payload_bytes;
        }
        Ok(EnqueueResult {
            deliveries,
            transmitted_bytes,
            lost_bytes,
            queue_occupancy_bytes: occupancy,
        })
    }

    /// Credit one scheduled delivery when its event executes.
    pub fn record_delivery(
        &mut self,
        delivery: &Delivery,
        useful_payload_bytes: u64,
    ) -> Result<(), LinkError> {
        let state = self
            .directions
            .get_mut(&(delivery.edge_id, delivery.from, delivery.to))
            .ok_or(LinkError::MissingDirection(delivery.edge_id))?;
        state.counters.delivered_bytes += delivery.wire_bytes;
        if delivery.class == LinkClass::UsefulPayload {
            state.counters.useful_payload_bytes += useful_payload_bytes;
        }
        Ok(())
    }

    /// Counters for a directed edge, or zeros if unused.
    pub fn counters(&self, edge: EdgeId, from: NodeId, to: NodeId) -> LinkCounters {
        self.directions
            .get(&(edge, from, to))
            .map(|state| state.counters.clone())
            .unwrap_or_default()
    }

    /// Stable projection of every used directed edge.
    pub fn all_counters(&self) -> BTreeMap<String, LinkCounters> {
        self.directions
            .iter()
            .map(|(&(edge, from, to), state)| {
                (format!("edge-{edge}:{from}->{to}"), state.counters.clone())
            })
            .collect()
    }

    /// At quiescence, transmitted bytes must equal delivered plus lost bytes.
    pub fn reconcile(&self) -> Result<(), LinkError> {
        for (&(edge, from, to), state) in &self.directions {
            if state.counters.transmitted_bytes
                != state.counters.delivered_bytes + state.counters.lost_bytes
            {
                return Err(LinkError::Reconciliation {
                    edge,
                    from,
                    to,
                    transmitted: state.counters.transmitted_bytes,
                    delivered: state.counters.delivered_bytes,
                    lost: state.counters.lost_bytes,
                });
            }
        }
        Ok(())
    }
}

/// Typed link outcome.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Edge has no configuration.
    #[error("unknown edge {0}")]
    UnknownEdge(EdgeId),
    /// Configured bandwidth cannot serve frames.
    #[error("edge {0} has zero bandwidth")]
    ZeroBandwidth(EdgeId),
    /// Effective frame exceeds the configured MTU.
    #[error("edge {edge} MTU exceeded: {frame_bytes} bytes > {mtu_bytes}")]
    MtuExceeded {
        /// Edge ID.
        edge: EdgeId,
        /// Frame plus lower-layer overhead.
        frame_bytes: u64,
        /// Effective MTU.
        mtu_bytes: u64,
    },
    /// Tail-drop queue overflow.
    #[error(
        "edge {edge} queue full: {queued_bytes} queued + {frame_bytes} frame bytes > {limit_bytes}"
    )]
    QueueFull {
        /// Edge ID.
        edge: EdgeId,
        /// Existing occupancy.
        queued_bytes: u64,
        /// New frame bytes.
        frame_bytes: u64,
        /// Queue capacity.
        limit_bytes: u64,
    },
    /// Delivery attempted without matching enqueue state.
    #[error("edge {0} has no directed runtime state")]
    MissingDirection(EdgeId),
    /// Integer time/byte arithmetic overflow.
    #[error("link arithmetic overflow")]
    Arithmetic,
    /// Quiescent counters failed exact reconciliation.
    #[error(
        "edge {edge} {from}->{to} does not reconcile: transmitted={transmitted}, delivered={delivered}, lost={lost}"
    )]
    Reconciliation {
        /// Edge ID.
        edge: EdgeId,
        /// Sender.
        from: NodeId,
        /// Receiver.
        to: NodeId,
        /// Serialized bytes.
        transmitted: u64,
        /// Delivered bytes.
        delivered: u64,
        /// Lost bytes.
        lost: u64,
    },
}

fn serialization_delay_ns(bytes: u64, bandwidth_bps: u64) -> Result<u64, LinkError> {
    let bit_nanos = bytes
        .checked_mul(8)
        .and_then(|bits| bits.checked_mul(NANOS_PER_SECOND))
        .ok_or(LinkError::Arithmetic)?;
    Ok(bit_nanos.div_ceil(bandwidth_bps))
}

fn draw_ppm(seed: u64, edge: EdgeId, from: NodeId, to: NodeId, sequence: u64, salt: u8) -> u32 {
    let mixed = seed
        ^ (u64::from(edge) << 40)
        ^ (u64::from(from) << 24)
        ^ (u64::from(to) << 8)
        ^ u64::from(salt);
    (deterministic_u64(mixed, sequence) % 1_000_000) as u32
}

fn deterministic_u64(seed: u64, ordinal: u64) -> u64 {
    let mut state = seed ^ ordinal.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut output = state;
    output = (output ^ (output >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    output = (output ^ (output >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    output ^ (output >> 31)
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
