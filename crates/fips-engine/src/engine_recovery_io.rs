use super::*;
use crate::{
    ESTABLISHED_FMP_OVERHEAD_BYTES, ResourceKind, SESSION_ACK_MESSAGE_BYTES, lookup_response_bytes,
};

impl Simulation {
    pub(super) fn handle_recovery_hop(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: RecoveryFrame,
    ) -> Result<Value, RunError> {
        let from = frame.path[frame.hop];
        let to = frame.path[frame.hop + 1];
        if !self.graph.is_active(from) || !self.graph.is_active(to) {
            return self.fail_recovery_frame(event, evidence, frame, "route endpoint inactive");
        }
        let edge = self.graph.edge_between(from, to).ok_or_else(|| {
            RunError::Invariant(format!("recovery path has no edge {from}->{to}"))
        })?;
        if !self.graph.is_edge_active(edge) {
            return self.fail_recovery_frame(event, evidence, frame, "route edge partitioned");
        }
        let link = self.links.config(edge)?.clone();
        let flow = self.traffic.as_ref().unwrap().plan.flows[frame.flow_index].clone();
        let cause = format!("{}:{}-{}", flow.id, frame.phase.message(), frame.attempt);
        match self.links.enqueue(EnqueueRequest {
            edge_id: edge,
            from,
            to,
            class: LinkClass::Control,
            frame_bytes: frame.frame_bytes,
            useful_payload_bytes: 0,
            now_ns: self.scheduler.now_ns(),
        }) {
            Ok(result) => {
                let runtime = self.recovery.as_mut().unwrap();
                runtime.counters.transmitted_wire_bytes += result.transmitted_bytes;
                runtime.counters.lost_wire_bytes += result.lost_bytes;
                self.add_ledger(&cause, "queued", frame.frame_bytes, evidence);
                self.add_ledger(&cause, "transmitted", result.transmitted_bytes, evidence);
                let forward_copy = result
                    .deliveries
                    .iter()
                    .map(|delivery| delivery.copy_ordinal)
                    .min();
                let deliveries = result
                    .deliveries
                    .iter()
                    .map(|delivery| {
                        json!({
                            "deliver_at_ns": delivery.deliver_at_ns,
                            "copy": delivery.copy_ordinal
                        })
                    })
                    .collect::<Vec<_>>();
                for delivery in result.deliveries {
                    self.scheduler.schedule_at(
                        delivery.deliver_at_ns,
                        Some(event),
                        SimEvent::DeliverRecovery {
                            delivery,
                            frame: frame.clone(),
                            forward_copy: forward_copy.unwrap_or(u8::MAX),
                        },
                    )?;
                }
                if forward_copy.is_none() {
                    let value = self.fail_recovery_frame(
                        event,
                        evidence,
                        frame.clone(),
                        "all link copies lost",
                    )?;
                    return Ok(recovery_json::enrich_rejection(
                        value,
                        &frame,
                        from,
                        to,
                        edge,
                        &link,
                        &self.transports.profile(from).name,
                        &self.transports.profile(to).name,
                    ));
                }
                Ok(json!({
                    "flow_id": flow.id, "message": frame.phase.message(),
                    "attempt": frame.attempt, "from": from, "to": to, "edge": edge,
                    "hop": frame.hop, "path": frame.path, "frame_bytes": frame.frame_bytes,
                    "transport_bytes": result.transmitted_bytes, "lost_bytes": result.lost_bytes,
                    "queue_occupancy_bytes": result.queue_occupancy_bytes,
                    "bandwidth_bps": link.bandwidth_bps, "latency_ns": link.latency_ns,
                    "mtu_bytes": link.mtu_bytes,
                    "from_transport": self.transports.profile(from).name,
                    "to_transport": self.transports.profile(to).name,
                    "deliveries": deliveries
                }))
            }
            Err(error @ (LinkError::MtuExceeded { .. } | LinkError::QueueFull { .. })) => {
                let value =
                    self.fail_recovery_frame(event, evidence, frame.clone(), &error.to_string())?;
                Ok(recovery_json::enrich_rejection(
                    value,
                    &frame,
                    from,
                    to,
                    edge,
                    &link,
                    &self.transports.profile(from).name,
                    &self.transports.profile(to).name,
                ))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn send_lookup_response(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: &RecoveryFrame,
    ) -> Result<(), RunError> {
        let destination = *frame.path.last().unwrap();
        let flow = self.traffic.as_ref().unwrap().plan.flows[frame.flow_index].clone();
        let cause = format!("{}:lookup-response-{}", flow.id, frame.attempt);
        let ready_at = match self.recovery.as_mut().unwrap().consume(
            &cause,
            destination,
            ResourceKind::Verifications,
            1,
            self.scheduler.now_ns(),
        ) {
            Ok(at) => at,
            Err(error) => {
                self.fail_lookup_attempt(
                    event,
                    evidence,
                    frame.flow_index,
                    frame.attempt,
                    &error.to_string(),
                )?;
                return Ok(());
            }
        };
        let depth = self.graph.ancestry(destination).len().saturating_sub(1) as u32;
        let frame_bytes = lookup_response_bytes(depth) + ESTABLISHED_FMP_OVERHEAD_BYTES;
        let mut path = frame.path.clone();
        path.reverse();
        self.add_ledger(&cause, "constructed", 1, evidence);
        self.add_ledger(&cause, "serialized", frame_bytes, evidence);
        self.scheduler.schedule_at(
            ready_at,
            Some(event),
            SimEvent::RecoveryHopDue {
                frame: RecoveryFrame {
                    flow_index: frame.flow_index,
                    attempt: frame.attempt,
                    phase: graph_recovery::RecoveryPhase::LookupResponse,
                    path,
                    hop: 0,
                    frame_bytes,
                },
            },
        )?;
        Ok(())
    }

    pub(super) fn complete_lookup(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: &RecoveryFrame,
    ) -> Result<(), RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[frame.flow_index].clone();
        let mut path = frame.path.clone();
        path.reverse();
        let destination = self.graph.address(flow.destination)?.0;
        let root = self.graph.address(self.graph.root(flow.destination))?.0;
        let runtime = self.recovery.as_mut().unwrap();
        runtime.counters.successes += 1;
        runtime.insert_cache(
            flow.source,
            destination,
            root,
            path.clone(),
            self.scheduler.now_ns(),
        );
        self.add_ledger(&flow.id, "cache-inserted", 1, evidence);
        self.begin_session_or_data(event, evidence, flow, path)?;
        Ok(())
    }

    pub(super) fn send_session_ack(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: &RecoveryFrame,
    ) -> Result<(), RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[frame.flow_index].clone();
        let destination = *frame.path.last().unwrap();
        let ready_at = match self.recovery.as_mut().unwrap().consume(
            &flow.id,
            destination,
            ResourceKind::Handshakes,
            1,
            self.scheduler.now_ns(),
        ) {
            Ok(at) => at,
            Err(error) => {
                self.reject_flow(&flow, &error.to_string(), evidence);
                return Ok(());
            }
        };
        let mut path = frame.path.clone();
        path.reverse();
        let frame_bytes = SESSION_ACK_MESSAGE_BYTES + ESTABLISHED_FMP_OVERHEAD_BYTES;
        self.recovery.as_mut().unwrap().counters.session_acks += 1;
        self.add_ledger(&flow.id, "session-ack-constructed", 1, evidence);
        self.scheduler.schedule_at(
            ready_at,
            Some(event),
            SimEvent::RecoveryHopDue {
                frame: RecoveryFrame {
                    flow_index: frame.flow_index,
                    attempt: 0,
                    phase: graph_recovery::RecoveryPhase::SessionAck,
                    path,
                    hop: 0,
                    frame_bytes,
                },
            },
        )?;
        Ok(())
    }

    pub(super) fn complete_session(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: &RecoveryFrame,
    ) -> Result<(), RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[frame.flow_index].clone();
        let mut path = frame.path.clone();
        path.reverse();
        let ready_at = match self.recovery.as_mut().unwrap().consume(
            &flow.id,
            flow.source,
            ResourceKind::Sessions,
            1,
            self.scheduler.now_ns(),
        ) {
            Ok(at) => at,
            Err(error) => {
                self.reject_flow(&flow, &error.to_string(), evidence);
                return Ok(());
            }
        };
        self.recovery
            .as_mut()
            .unwrap()
            .insert_session(flow.source, flow.destination, path.clone());
        self.schedule_data_frame_at(ready_at, event, evidence, flow, path)
    }

    fn fail_recovery_frame(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: RecoveryFrame,
        reason: &str,
    ) -> Result<Value, RunError> {
        if frame.phase.plane() == "lookup" {
            return self.fail_lookup_attempt(
                event,
                evidence,
                frame.flow_index,
                frame.attempt,
                reason,
            );
        }
        let flow = self.traffic.as_ref().unwrap().plan.flows[frame.flow_index].clone();
        self.reject_flow(&flow, reason, evidence);
        Ok(json!({
            "flow_id": flow.id, "message": frame.phase.message(),
            "attempt": frame.attempt, "path": frame.path, "rejected": reason
        }))
    }
}
