use super::*;
use crate::{Flow, FlowShape, SessionAction};

const MAX_ROUTED_FLOWS: usize = 100_000;

impl Simulation {
    pub(super) fn handle_cache_expiry(
        &mut self,
        evidence: &str,
        input: CacheExpiryInput,
    ) -> Result<Value, RunError> {
        let invalidated = self
            .recovery
            .as_mut()
            .ok_or_else(|| RunError::Unsupported("lookup runtime is unavailable".to_owned()))?
            .invalidate_all_caches();
        let cause = format!("input:{}", input.id);
        self.add_ledger(&cause, "performed", 1, evidence);
        self.add_ledger(&cause, "invalidated", invalidated, evidence);
        Ok(json!({
            "intervention_id": input.id,
            "invalidated_entries": invalidated,
            "scope": "all-coordinate-caches",
            "fidelity": "semantic-exact"
        }))
    }

    pub(super) fn handle_lookup_wave(
        &mut self,
        event: EventId,
        evidence: &str,
        input: LookupWaveInput,
    ) -> Result<Value, RunError> {
        let runtime = self.traffic.as_mut().ok_or_else(|| {
            RunError::Unsupported("lookup wave needs traffic endpoints".to_owned())
        })?;
        let templates = runtime.plan.flows.clone();
        if templates.is_empty() {
            return Err(RunError::Unsupported(
                "lookup wave needs at least one traffic endpoint pair".to_owned(),
            ));
        }
        let count = usize::try_from(input.count.unwrap_or(templates.len() as u64))
            .map_err(|_| RunError::Arithmetic)?;
        let start = runtime.plan.flows.len();
        let final_len = start.checked_add(count).ok_or(RunError::Arithmetic)?;
        if final_len > MAX_ROUTED_FLOWS {
            return Err(RunError::Unsupported(format!(
                "lookup wave would raise routed flow count to {final_len}; limit is {MAX_ROUTED_FLOWS}"
            )));
        }
        let useful_payload_bytes = templates[0].useful_payload_bytes;
        for ordinal in 0..count {
            let template = &templates[ordinal % templates.len()];
            runtime.plan.flows.push(Flow {
                id: format!("lookup-wave-{}-{ordinal:08}", input.id),
                source: template.source,
                destination: template.destination,
                offered_at_ns: 0,
                useful_payload_bytes,
                session_action: SessionAction::Reuse,
                shape: FlowShape::Single,
            });
        }
        runtime.plan.offered_useful_bytes = runtime
            .plan
            .offered_useful_bytes
            .saturating_add(useful_payload_bytes.saturating_mul(count as u64));
        let now = self.scheduler.now_ns();
        for index in start..final_len {
            self.scheduler
                .schedule_at(now, Some(event), SimEvent::TrafficOffer { index })?;
        }
        let cause = format!("input:{}", input.id);
        self.add_ledger(&cause, "requested", count as u64, evidence);
        self.add_ledger(&cause, "performed", count as u64, evidence);
        Ok(json!({
            "intervention_id": input.id,
            "scheduled_lookups": count,
            "offered_at_ns": now,
            "first_probe": format!("lookup-wave-{}-00000000", input.id),
            "last_probe": format!("lookup-wave-{}-{:08}", input.id, count - 1),
            "fidelity": "semantic-exact"
        }))
    }
}
