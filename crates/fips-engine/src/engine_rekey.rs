use super::*;
use crate::ResourceKind;

impl Simulation {
    pub(super) fn handle_session_rekey(
        &mut self,
        event: EventId,
        evidence: &str,
        input: SessionRekeyInput,
    ) -> Result<Value, RunError> {
        let cause = format!("input:{}", input.id);
        let now = self.scheduler.now_ns();
        let sessions = self
            .recovery
            .as_ref()
            .ok_or_else(|| {
                RunError::Unsupported("session rekey runtime is unavailable".to_owned())
            })?
            .session_snapshot();
        self.add_ledger(&cause, "performed", 1, evidence);
        self.add_ledger(&cause, "requested", sessions.len() as u64, evidence);
        let mut scheduled = 0_u64;
        let mut rejected = 0_u64;
        let mut first_completion = None;
        let mut last_completion = None;
        for (source, destination, path) in sessions {
            let session_cause = format!("{cause}:session-{source}-{destination}");
            let ready = self.recovery.as_mut().unwrap().consume(
                &session_cause,
                source,
                ResourceKind::Hashes,
                2,
                now,
            );
            match ready {
                Ok(ready_at) => {
                    self.add_ledger(&session_cause, "compute", 2, evidence);
                    self.scheduler.schedule_at(
                        ready_at,
                        Some(event),
                        SimEvent::SessionRekeyCompleted {
                            completion: SessionRekeyCompletion {
                                cause: session_cause,
                                source,
                                destination,
                                path,
                                requested_at_ns: now,
                            },
                        },
                    )?;
                    scheduled += 1;
                    first_completion =
                        Some(first_completion.map_or(ready_at, |at: u64| at.min(ready_at)));
                    last_completion =
                        Some(last_completion.map_or(ready_at, |at: u64| at.max(ready_at)));
                }
                Err(error) => {
                    rejected += 1;
                    self.add_ledger(&session_cause, "rejected", 1, &error.to_string());
                }
            }
        }
        Ok(json!({
            "intervention_id": input.id,
            "requested_sessions": scheduled + rejected,
            "scheduled_rekeys": scheduled,
            "rejected_rekeys": rejected,
            "first_completion_ns": first_completion,
            "last_completion_ns": last_completion,
            "crypto_fidelity": "operation-counted-no-wire-frame"
        }))
    }

    pub(super) fn handle_session_rekey_completed(
        &mut self,
        evidence: &str,
        completion: SessionRekeyCompletion,
    ) -> Result<Value, RunError> {
        let retained = self
            .recovery
            .as_ref()
            .unwrap()
            .has_session(completion.source, completion.destination);
        self.recovery.as_mut().unwrap().counters.rekeys += 1;
        self.add_ledger(&completion.cause, "performed", 1, evidence);
        if !retained {
            self.add_ledger(&completion.cause, "superseded", 1, evidence);
        }
        Ok(json!({
            "causal_id": completion.cause,
            "source": completion.source,
            "destination": completion.destination,
            "path": completion.path,
            "requested_at_ns": completion.requested_at_ns,
            "completed_at_ns": self.scheduler.now_ns(),
            "session_retained": retained,
            "crypto_fidelity": "operation-counted-no-wire-frame"
        }))
    }
}
