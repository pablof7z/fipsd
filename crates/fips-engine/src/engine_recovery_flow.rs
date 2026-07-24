use super::*;
use crate::{
    ESTABLISHED_FMP_OVERHEAD_BYTES, Flow, ResourceKind, SESSION_SETUP_MESSAGE_BYTES, SessionAction,
    lookup_request_bytes,
};

impl Simulation {
    pub(super) fn begin_recovery_for_flow(
        &mut self,
        event: EventId,
        evidence: &str,
        index: usize,
        path: Vec<NodeId>,
    ) -> Result<Value, RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[index].clone();
        let destination = self.graph.address(flow.destination)?.0;
        let cached = self.recovery.as_mut().unwrap().is_cached(
            flow.source,
            destination,
            self.scheduler.now_ns(),
        );
        if cached {
            self.add_ledger(&flow.id, "cache-hit", 1, evidence);
            let action = self.begin_session_or_data(event, evidence, flow.clone(), path.clone())?;
            return Ok(recovery_offer_json(&flow, &path, "cache-hit", action));
        }
        self.recovery.as_mut().unwrap().counters.lookups += 1;
        self.add_ledger(&flow.id, "cache-miss", 1, evidence);
        self.scheduler.schedule_at(
            self.scheduler.now_ns(),
            Some(event),
            SimEvent::LookupStart { index, attempt: 0 },
        )?;
        Ok(recovery_offer_json(&flow, &path, "cache-miss", "lookup"))
    }

    pub(super) fn handle_lookup_start(
        &mut self,
        event: EventId,
        evidence: &str,
        index: usize,
        attempt: u32,
    ) -> Result<Value, RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[index].clone();
        let path = self.shortest_active_path(flow.source, flow.destination);
        {
            let counters = &mut self.recovery.as_mut().unwrap().counters;
            counters.attempts += 1;
            counters.retries += u64::from(attempt > 0);
        }
        let Some(path) = path else {
            return self.fail_lookup_attempt(event, evidence, index, attempt, "no active route");
        };
        let ttl = usize::from(self.recovery.as_ref().unwrap().ttl);
        if path.len().saturating_sub(1) > ttl {
            return self.fail_lookup_attempt(event, evidence, index, attempt, "TTL expired");
        }
        let cause = format!("{}:lookup-{attempt}", flow.id);
        let ready_at = match self.recovery.as_mut().unwrap().consume(
            &cause,
            flow.source,
            ResourceKind::Hashes,
            1,
            self.scheduler.now_ns(),
        ) {
            Ok(at) => at,
            Err(error) => {
                return self.fail_lookup_attempt(
                    event,
                    evidence,
                    index,
                    attempt,
                    &error.to_string(),
                );
            }
        };
        let depth = self.graph.ancestry(flow.source).len().saturating_sub(1) as u32;
        let frame_bytes = lookup_request_bytes(depth)
            .checked_add(ESTABLISHED_FMP_OVERHEAD_BYTES)
            .ok_or(RunError::Arithmetic)?;
        self.add_ledger(&cause, "constructed", 1, evidence);
        self.add_ledger(&cause, "serialized", frame_bytes, evidence);
        self.scheduler.schedule_at(
            ready_at,
            Some(event),
            SimEvent::RecoveryHopDue {
                frame: RecoveryFrame {
                    flow_index: index,
                    attempt,
                    phase: graph_recovery::RecoveryPhase::LookupRequest,
                    path: path.clone(),
                    hop: 0,
                    frame_bytes,
                },
            },
        )?;
        Ok(json!({
            "flow_id": flow.id, "attempt": attempt, "ttl": ttl,
            "path": path, "message": "lookup-request", "frame_bytes": frame_bytes,
            "resource_ready_at_ns": ready_at
        }))
    }

    pub(super) fn fail_lookup_attempt(
        &mut self,
        event: EventId,
        evidence: &str,
        index: usize,
        attempt: u32,
        reason: &str,
    ) -> Result<Value, RunError> {
        let flow = self.traffic.as_ref().unwrap().plan.flows[index].clone();
        let runtime = self.recovery.as_ref().unwrap();
        let next = attempt.saturating_add(1);
        if next < runtime.maximum_attempts {
            let exponent = 1_u64 << next.min(31);
            let jitter = stable_jitter(self.plan.seed, index, next, runtime.jitter_ns);
            let at = self
                .scheduler
                .now_ns()
                .saturating_add(runtime.backoff_base_ns.saturating_mul(exponent))
                .saturating_add(jitter);
            self.scheduler.schedule_at(
                at,
                Some(event),
                SimEvent::LookupStart {
                    index,
                    attempt: next,
                },
            )?;
            self.add_ledger(&flow.id, "retry-scheduled", 1, evidence);
            return Ok(json!({
                "flow_id": flow.id, "attempt": attempt, "outcome": "retry",
                "reason": reason, "retry_at_ns": at
            }));
        }
        self.recovery.as_mut().unwrap().counters.failures += 1;
        self.reject_flow(&flow, reason, evidence);
        Ok(json!({
            "flow_id": flow.id, "attempt": attempt, "outcome": "failed",
            "reason": reason
        }))
    }

    pub(super) fn begin_session_or_data(
        &mut self,
        event: EventId,
        evidence: &str,
        flow: Flow,
        path: Vec<NodeId>,
    ) -> Result<&'static str, RunError> {
        let has_session = self
            .recovery
            .as_ref()
            .unwrap()
            .has_session(flow.source, flow.destination);
        if has_session {
            let ready_at = if flow.session_action == SessionAction::Rekey {
                self.recovery.as_mut().unwrap().counters.rekeys += 1;
                match self.recovery.as_mut().unwrap().consume(
                    &flow.id,
                    flow.source,
                    ResourceKind::Hashes,
                    2,
                    self.scheduler.now_ns(),
                ) {
                    Ok(at) => at,
                    Err(error) => {
                        self.reject_flow(&flow, &error.to_string(), evidence);
                        return Ok("resource-rejected");
                    }
                }
            } else {
                self.scheduler.now_ns()
            };
            self.schedule_data_frame_at(ready_at, event, evidence, flow, path)?;
            return Ok(if ready_at == self.scheduler.now_ns() {
                "reuse"
            } else {
                "rekey"
            });
        }
        let ready_at = match self.recovery.as_mut().unwrap().consume(
            &flow.id,
            flow.source,
            ResourceKind::Handshakes,
            1,
            self.scheduler.now_ns(),
        ) {
            Ok(at) => at,
            Err(error) => {
                self.reject_flow(&flow, &error.to_string(), evidence);
                return Ok("resource-rejected");
            }
        };
        let frame_bytes = SESSION_SETUP_MESSAGE_BYTES + ESTABLISHED_FMP_OVERHEAD_BYTES;
        self.recovery.as_mut().unwrap().counters.session_setups += 1;
        self.add_ledger(&flow.id, "session-setup-constructed", 1, evidence);
        self.add_ledger(&flow.id, "serialized", frame_bytes, evidence);
        self.scheduler.schedule_at(
            ready_at,
            Some(event),
            SimEvent::RecoveryHopDue {
                frame: RecoveryFrame {
                    flow_index: self.flow_index(&flow.id)?,
                    attempt: 0,
                    phase: graph_recovery::RecoveryPhase::SessionSetup,
                    path,
                    hop: 0,
                    frame_bytes,
                },
            },
        )?;
        Ok("setup")
    }

    fn flow_index(&self, id: &str) -> Result<usize, RunError> {
        self.traffic
            .as_ref()
            .unwrap()
            .plan
            .flows
            .iter()
            .position(|flow| flow.id == id)
            .ok_or_else(|| RunError::Invariant(format!("unknown flow {id}")))
    }
}

fn stable_jitter(seed: u64, index: usize, attempt: u32, maximum: u64) -> u64 {
    if maximum == 0 {
        return 0;
    }
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(index.to_le_bytes());
    hasher.update(attempt.to_le_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap()) % maximum
}

fn recovery_offer_json(flow: &Flow, path: &[NodeId], cache: &str, action: &str) -> Value {
    json!({
        "flow_id": flow.id, "source": flow.source, "destination": flow.destination,
        "useful_bytes": flow.useful_payload_bytes, "path": path,
        "cache": cache, "next_action": action
    })
}
