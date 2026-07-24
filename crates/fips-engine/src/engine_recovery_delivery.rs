use super::*;

impl Simulation {
    pub(super) fn handle_recovery_delivery(
        &mut self,
        event: EventId,
        evidence: &str,
        delivery: Delivery,
        mut frame: RecoveryFrame,
        forward_copy: u8,
    ) -> Result<Value, RunError> {
        self.links.record_delivery(&delivery, 0)?;
        self.recovery
            .as_mut()
            .unwrap()
            .counters
            .delivered_wire_bytes += delivery.wire_bytes;
        let selected = delivery.copy_ordinal == forward_copy;
        let final_hop = frame.hop + 2 == frame.path.len();
        let delivered_hop = frame.hop;
        if selected && !final_hop {
            frame.hop += 1;
            self.scheduler.schedule_at(
                self.scheduler.now_ns(),
                Some(event),
                SimEvent::RecoveryHopDue {
                    frame: frame.clone(),
                },
            )?;
        } else if selected {
            self.finish_recovery_frame(event, evidence, &frame)?;
        }
        let flow = &self.traffic.as_ref().unwrap().plan.flows[frame.flow_index];
        Ok(json!({
            "flow_id": flow.id, "message": frame.phase.message(),
            "attempt": frame.attempt, "from": delivery.from, "to": delivery.to,
            "edge": delivery.edge_id, "hop": delivered_hop, "path": frame.path,
            "frame_bytes": delivery.frame_bytes, "wire_bytes": delivery.wire_bytes,
            "copy": delivery.copy_ordinal, "forwarded": selected && !final_hop,
            "final": selected && final_hop
        }))
    }

    fn finish_recovery_frame(
        &mut self,
        event: EventId,
        evidence: &str,
        frame: &RecoveryFrame,
    ) -> Result<(), RunError> {
        match frame.phase {
            graph_recovery::RecoveryPhase::LookupRequest => {
                self.send_lookup_response(event, evidence, frame)
            }
            graph_recovery::RecoveryPhase::LookupResponse => {
                self.complete_lookup(event, evidence, frame)
            }
            graph_recovery::RecoveryPhase::SessionSetup => {
                self.send_session_ack(event, evidence, frame)
            }
            graph_recovery::RecoveryPhase::SessionAck => {
                self.complete_session(event, evidence, frame)
            }
        }
    }
}
