use super::*;
use crate::{StreamEnqueueRequest, StreamEnqueueResult};

impl Simulation {
    pub(super) fn enqueue_transfer_chunk(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: RoutedFrame,
        edge: EdgeId,
        link: &LinkConfig,
    ) -> Result<Value, RunError> {
        let from = frame.path[frame.hop];
        let to = frame.path[frame.hop + 1];
        let result = self.links.enqueue_stream(StreamEnqueueRequest {
            edge_id: edge,
            from,
            to,
            useful_payload_bytes: frame.flow.useful_payload_bytes,
            protocol_overhead_bytes: SESSION_DATA_OVERHEAD_BYTES,
            now_ns: self.scheduler.now_ns(),
        })?;
        self.record_transfer_enqueue(&frame.flow.id, evidence, &result);
        let delivery = result.delivery.clone();
        self.scheduler.schedule_at(
            delivery.deliver_at_ns,
            Some(event),
            SimEvent::DeliverTraffic {
                delivery: delivery.clone(),
                frame: frame.clone(),
                forward_copy: 0,
            },
        )?;
        Ok(json!({
            "flow_id": frame.flow.id,
            "message": "application-transfer-chunk",
            "from": from, "to": to, "edge": edge, "hop": frame.hop,
            "path": frame.path, "frame_bytes": delivery.frame_bytes,
            "useful_bytes": frame.flow.useful_payload_bytes,
            "shape": frame.flow.shape,
            "transport_bytes": result.transmitted_bytes,
            "lost_bytes": result.lost_bytes,
            "packet_count": result.packet_count,
            "retransmitted_packets": result.retransmitted_packets,
            "queue_occupancy_bytes": result.queue_occupancy_bytes,
            "bandwidth_bps": link.bandwidth_bps,
            "latency_ns": link.latency_ns,
            "mtu_bytes": link.mtu_bytes,
            "from_transport": self.transports.profile(from).name,
            "to_transport": self.transports.profile(to).name,
            "deliveries": [delivery_json(&delivery)]
        }))
    }

    fn record_transfer_enqueue(
        &mut self,
        flow_id: &str,
        evidence: &str,
        result: &StreamEnqueueResult,
    ) {
        self.traffic
            .as_mut()
            .unwrap()
            .counters
            .transmitted_wire_bytes += result.transmitted_bytes;
        self.add_ledger(flow_id, "packetized", result.packet_count, evidence);
        self.add_ledger(flow_id, "serialized", result.delivery.frame_bytes, evidence);
        self.add_ledger(
            flow_id,
            "retransmitted",
            result.retransmitted_packets,
            evidence,
        );
        self.add_ledger(flow_id, "queued", result.queue_occupancy_bytes, evidence);
        self.add_ledger(flow_id, "transmitted", result.transmitted_bytes, evidence);
    }
}
