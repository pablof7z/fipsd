use super::*;

/// One visible application byte range offered to a reliable modeled stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEnqueueRequest {
    pub edge_id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub useful_payload_bytes: u64,
    pub protocol_overhead_bytes: u64,
    pub now_ns: u64,
}

/// Aggregated packetization result for one visible stream chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEnqueueResult {
    pub delivery: Delivery,
    pub transmitted_bytes: u64,
    pub lost_bytes: u64,
    pub packet_count: u64,
    pub retransmitted_packets: u64,
    pub queue_occupancy_bytes: u64,
}

impl LinkService {
    /// Packetize and serialize a reliable application chunk without emitting one event per packet.
    pub fn enqueue_stream(
        &mut self,
        request: StreamEnqueueRequest,
    ) -> Result<StreamEnqueueResult, LinkError> {
        let config = self.config(request.edge_id)?.clone();
        if config.bandwidth_bps == 0 {
            return Err(LinkError::ZeroBandwidth(request.edge_id));
        }
        let per_packet_overhead = request
            .protocol_overhead_bytes
            .checked_add(config.transport_overhead_bytes)
            .ok_or(LinkError::Arithmetic)?;
        let payload_per_packet =
            config
                .mtu_bytes
                .checked_sub(per_packet_overhead)
                .ok_or(LinkError::MtuExceeded {
                    edge: request.edge_id,
                    frame_bytes: per_packet_overhead,
                    mtu_bytes: config.mtu_bytes,
                })?;
        if payload_per_packet == 0 {
            return Err(LinkError::MtuExceeded {
                edge: request.edge_id,
                frame_bytes: per_packet_overhead.saturating_add(1),
                mtu_bytes: config.mtu_bytes,
            });
        }
        let packet_count = request.useful_payload_bytes.div_ceil(payload_per_packet);
        let base_overhead = packet_count
            .checked_mul(per_packet_overhead)
            .ok_or(LinkError::Arithmetic)?;
        let delivered_wire_bytes = request
            .useful_payload_bytes
            .checked_add(base_overhead)
            .ok_or(LinkError::Arithmetic)?;
        let retransmitted_packets = projected_retransmissions(packet_count, config.loss_ppm)?;
        let average_packet_bytes = delivered_wire_bytes.div_ceil(packet_count.max(1));
        let lost_bytes = retransmitted_packets
            .checked_mul(average_packet_bytes)
            .ok_or(LinkError::Arithmetic)?;
        let transmitted_bytes = delivered_wire_bytes
            .checked_add(lost_bytes)
            .ok_or(LinkError::Arithmetic)?;
        let key = match self.shared_groups[request.edge_id as usize] {
            Some(group) => (u64::from(group) | (1_u64 << 63), NodeId::MAX, NodeId::MAX),
            None => (u64::from(request.edge_id), request.from, request.to),
        };
        let capacity = self.capacities.entry(key).or_default();
        while capacity
            .queued
            .front()
            .is_some_and(|(complete, _)| *complete <= request.now_ns)
        {
            capacity.queued.pop_front();
        }
        let queued_bytes = capacity.queued.iter().map(|(_, bytes)| bytes).sum::<u64>();
        let queue_occupancy_bytes = config
            .queue_bytes
            .min(queued_bytes.saturating_add(transmitted_bytes));
        let queued_delta = queue_occupancy_bytes.saturating_sub(queued_bytes);
        let serialization_start = capacity.next_serialization_ns.max(request.now_ns);
        let serialization_ns = serialization_time(transmitted_bytes, config.bandwidth_bps)?;
        let serialization_end = serialization_start
            .checked_add(serialization_ns)
            .ok_or(LinkError::Arithmetic)?;
        capacity.next_serialization_ns = serialization_end;
        capacity.queued.push_back((serialization_end, queued_delta));

        let state = self
            .directions
            .entry((request.edge_id, request.from, request.to))
            .or_default();
        let sequence = state.frame_sequence;
        state.frame_sequence = state
            .frame_sequence
            .saturating_add(packet_count)
            .saturating_add(retransmitted_packets);
        state.counters.accepted_frames =
            state.counters.accepted_frames.saturating_add(packet_count);
        state.counters.transmitted_bytes = state
            .counters
            .transmitted_bytes
            .saturating_add(transmitted_bytes);
        state.counters.lost_bytes = state.counters.lost_bytes.saturating_add(lost_bytes);
        state.counters.peak_queue_bytes =
            state.counters.peak_queue_bytes.max(queue_occupancy_bytes);
        let jitter = stream_jitter(self.seed, request.edge_id, sequence, config.jitter_ns);
        let deliver_at_ns = serialization_end
            .saturating_add(config.latency_ns)
            .saturating_add(jitter)
            .max(state.last_stream_delivery_ns);
        state.last_stream_delivery_ns = deliver_at_ns;
        let frame_bytes = request
            .useful_payload_bytes
            .saturating_add(packet_count.saturating_mul(request.protocol_overhead_bytes));
        Ok(StreamEnqueueResult {
            delivery: Delivery {
                edge_id: request.edge_id,
                from: request.from,
                to: request.to,
                class: LinkClass::UsefulPayload,
                frame_bytes,
                wire_bytes: delivered_wire_bytes,
                deliver_at_ns,
                copy_ordinal: 0,
            },
            transmitted_bytes,
            lost_bytes,
            packet_count,
            retransmitted_packets,
            queue_occupancy_bytes,
        })
    }
}

fn projected_retransmissions(packet_count: u64, loss_ppm: u32) -> Result<u64, LinkError> {
    if loss_ppm == 0 {
        return Ok(0);
    }
    if loss_ppm >= 1_000_000 {
        return Err(LinkError::Arithmetic);
    }
    let numerator = u128::from(packet_count) * u128::from(loss_ppm);
    let denominator = u128::from(1_000_000 - loss_ppm);
    u64::try_from(numerator.div_ceil(denominator)).map_err(|_| LinkError::Arithmetic)
}

fn serialization_time(bytes: u64, bandwidth_bps: u64) -> Result<u64, LinkError> {
    let bits = u128::from(bytes)
        .checked_mul(8)
        .ok_or(LinkError::Arithmetic)?;
    let nanos = bits
        .checked_mul(u128::from(NANOS_PER_SECOND))
        .ok_or(LinkError::Arithmetic)?
        .div_ceil(u128::from(bandwidth_bps));
    u64::try_from(nanos.max(1)).map_err(|_| LinkError::Arithmetic)
}

fn stream_jitter(seed: u64, edge: EdgeId, sequence: u64, maximum: u64) -> u64 {
    if maximum == 0 {
        return 0;
    }
    deterministic_u64(seed ^ (u64::from(edge) << 32) ^ 0xA7, sequence) % maximum
}
