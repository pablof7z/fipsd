use super::*;
use crate::Flow;

#[path = "engine_data_config.rs"]
mod config;
#[path = "engine_data_json.rs"]
mod json_helpers;
#[path = "engine_data_reject.rs"]
mod reject;
#[path = "engine_data_transfer.rs"]
mod transfer;
#[path = "engine_data_types.rs"]
mod types;
use json_helpers::{delivery_json, flow_json, hop_json, rejected_hop_json};
pub use types::RoutedTrafficCounters;
pub(super) use types::{RoutedFrame, RoutedTrafficRuntime, SESSION_DATA_OVERHEAD_BYTES};

impl Simulation {
    pub(super) fn schedule_traffic(&mut self) -> Result<(), RunError> {
        let Some(runtime) = &self.traffic else {
            return Ok(());
        };
        let start = runtime.start_ns;
        let times = runtime
            .plan
            .flows
            .iter()
            .map(|flow| start.saturating_add(flow.offered_at_ns))
            .collect::<Vec<_>>();
        for (index, at) in times.into_iter().enumerate() {
            self.scheduler
                .schedule_at(at, None, SimEvent::TrafficOffer { index })?;
        }
        Ok(())
    }

    pub(super) fn handle_traffic_offer(
        &mut self,
        event: EventId,
        evidence: &str,
        index: usize,
    ) -> Result<Value, RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[index].clone();
        let path = self.shortest_active_path(flow.source, flow.destination);
        {
            let counters = &mut self.traffic.as_mut().unwrap().counters;
            counters.offered_flows += 1;
            counters.offered_useful_bytes += flow.useful_payload_bytes;
        }
        self.add_ledger(&flow.id, "performed", 1, evidence);
        self.add_ledger(&flow.id, "payload", flow.useful_payload_bytes, evidence);
        let Some(path) = path else {
            self.reject_flow(&flow, "no active route", evidence);
            return Ok(flow_json(&flow, &[], "rejected"));
        };
        let counters = &mut self.traffic.as_mut().unwrap().counters;
        counters.maximum_hops = counters
            .maximum_hops
            .max(path.len().saturating_sub(1) as u64);
        if self.recovery.is_some() {
            return self.begin_recovery_for_flow(event, evidence, index, path);
        }
        self.schedule_data_frame(event, evidence, flow.clone(), path.clone())?;
        Ok(flow_json(&flow, &path, "routed"))
    }

    pub(super) fn schedule_data_frame(
        &mut self,
        event: EventId,
        evidence: &str,
        flow: Flow,
        path: Vec<NodeId>,
    ) -> Result<(), RunError> {
        self.schedule_data_frame_at(self.scheduler.now_ns(), event, evidence, flow, path)
    }

    pub(super) fn schedule_data_frame_at(
        &mut self,
        at_ns: u64,
        event: EventId,
        evidence: &str,
        flow: Flow,
        path: Vec<NodeId>,
    ) -> Result<(), RunError> {
        let frame_bytes = flow
            .useful_payload_bytes
            .checked_add(SESSION_DATA_OVERHEAD_BYTES)
            .ok_or(RunError::Arithmetic)?;
        self.add_ledger(&flow.id, "constructed", 1, evidence);
        if !matches!(&flow.shape, crate::FlowShape::ApplicationTransfer { .. }) {
            self.add_ledger(&flow.id, "serialized", frame_bytes, evidence);
        }
        self.scheduler.schedule_at(
            at_ns,
            Some(event),
            SimEvent::TrafficHopDue {
                frame: RoutedFrame {
                    flow: flow.clone(),
                    path: path.clone(),
                    hop: 0,
                    frame_bytes,
                },
            },
        )?;
        Ok(())
    }

    pub(super) fn handle_traffic_hop(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: RoutedFrame,
    ) -> Result<Value, RunError> {
        let from = frame.path[frame.hop];
        let to = frame.path[frame.hop + 1];
        if !self.graph.is_active(from) || !self.graph.is_active(to) {
            self.reject_flow(&frame.flow, "route endpoint inactive", evidence);
            return Ok(hop_json(&frame, from, to, json!([]), Some("inactive")));
        }
        let edge = self
            .graph
            .edge_between(from, to)
            .ok_or_else(|| RunError::Invariant(format!("routed flow has no edge {from}->{to}")))?;
        if !self.graph.is_edge_active(edge) {
            self.reject_flow(&frame.flow, "route edge partitioned", evidence);
            return Ok(hop_json(&frame, from, to, json!([]), Some("partitioned")));
        }
        let link = self.links.config(edge)?.clone();
        if matches!(
            &frame.flow.shape,
            crate::FlowShape::ApplicationTransfer { .. }
        ) {
            return self.enqueue_transfer_chunk(event, evidence, frame, edge, &link);
        }
        match self.links.enqueue(EnqueueRequest {
            edge_id: edge,
            from,
            to,
            class: LinkClass::UsefulPayload,
            frame_bytes: frame.frame_bytes,
            useful_payload_bytes: frame.flow.useful_payload_bytes,
            now_ns: self.scheduler.now_ns(),
        }) {
            Ok(result) => {
                self.traffic
                    .as_mut()
                    .unwrap()
                    .counters
                    .transmitted_wire_bytes += result.transmitted_bytes;
                self.add_ledger(&frame.flow.id, "queued", frame.frame_bytes, evidence);
                self.add_ledger(
                    &frame.flow.id,
                    "transmitted",
                    result.transmitted_bytes,
                    evidence,
                );
                let forward_copy = result
                    .deliveries
                    .iter()
                    .map(|delivery| delivery.copy_ordinal)
                    .min();
                let deliveries = result
                    .deliveries
                    .iter()
                    .map(delivery_json)
                    .collect::<Vec<_>>();
                for delivery in result.deliveries {
                    self.scheduler.schedule_at(
                        delivery.deliver_at_ns,
                        Some(event),
                        SimEvent::DeliverTraffic {
                            delivery,
                            frame: frame.clone(),
                            forward_copy: forward_copy.unwrap_or(u8::MAX),
                        },
                    )?;
                }
                if forward_copy.is_none() {
                    self.reject_flow(&frame.flow, "all link copies lost", evidence);
                }
                Ok(json!({
                    "flow_id": frame.flow.id, "message": "session-data",
                    "from": from, "to": to, "edge": edge, "hop": frame.hop,
                    "path": frame.path, "frame_bytes": frame.frame_bytes,
                    "useful_bytes": frame.flow.useful_payload_bytes,
                    "shape": frame.flow.shape,
                    "transport_bytes": result.transmitted_bytes,
                    "lost_bytes": result.lost_bytes,
                    "queue_occupancy_bytes": result.queue_occupancy_bytes,
                    "bandwidth_bps": link.bandwidth_bps, "latency_ns": link.latency_ns,
                    "mtu_bytes": link.mtu_bytes,
                    "from_transport": self.transports.profile(from).name,
                    "to_transport": self.transports.profile(to).name,
                    "deliveries": deliveries
                }))
            }
            Err(error @ (LinkError::MtuExceeded { .. } | LinkError::QueueFull { .. })) => {
                self.reject_flow(&frame.flow, &error.to_string(), evidence);
                Ok(rejected_hop_json(
                    &frame,
                    from,
                    to,
                    edge,
                    &link,
                    &error.to_string(),
                ))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn handle_traffic_delivery(
        &mut self,
        event: EventId,
        evidence: &str,
        delivery: Delivery,
        mut frame: RoutedFrame,
        forward_copy: u8,
    ) -> Result<Value, RunError> {
        let delivered_hop = frame.hop;
        let selected = delivery.copy_ordinal == forward_copy;
        let final_hop = frame.hop + 2 == frame.path.len();
        let useful = if selected && final_hop {
            frame.flow.useful_payload_bytes
        } else {
            0
        };
        self.links.record_delivery(&delivery, useful)?;
        self.traffic.as_mut().unwrap().counters.delivered_wire_bytes += delivery.wire_bytes;
        self.add_ledger(&frame.flow.id, "delivered", delivery.wire_bytes, evidence);
        if selected && final_hop {
            let now = self.scheduler.now_ns();
            let runtime = self.traffic.as_mut().unwrap();
            let prior = runtime.last_useful_delivery_ns.unwrap_or(runtime.start_ns);
            runtime.counters.goodput_stall_ns = runtime
                .counters
                .goodput_stall_ns
                .max(now.saturating_sub(prior));
            runtime.last_useful_delivery_ns = Some(now);
            let counters = &mut runtime.counters;
            counters.delivered_flows += 1;
            counters.delivered_useful_bytes += useful;
            self.add_ledger(&frame.flow.id, "useful-payload", useful, evidence);
            if let Some(runtime) = self
                .recovery
                .as_mut()
                .filter(|_| frame.flow.session_action == crate::SessionAction::Teardown)
            {
                if runtime.remove_session(frame.flow.source, frame.flow.destination) {
                    runtime.counters.teardowns += 1;
                }
            }
        } else if selected {
            frame.hop += 1;
            self.scheduler.schedule_at(
                self.scheduler.now_ns(),
                Some(event),
                SimEvent::TrafficHopDue {
                    frame: frame.clone(),
                },
            )?;
        }
        Ok(json!({
            "flow_id": frame.flow.id, "message": "session-data",
            "from": delivery.from, "to": delivery.to, "edge": delivery.edge_id,
            "hop": delivered_hop, "path": frame.path, "frame_bytes": delivery.frame_bytes,
            "wire_bytes": delivery.wire_bytes, "copy": delivery.copy_ordinal,
            "forwarded": selected && !final_hop, "final": selected && final_hop,
            "useful_bytes": useful, "shape": frame.flow.shape
        }))
    }
}

#[cfg(test)]
#[path = "engine_data_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "engine_transfer_tests.rs"]
mod transfer_tests;
