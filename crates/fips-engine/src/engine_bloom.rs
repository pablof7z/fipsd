use super::*;
use crate::bloom::{
    BloomMode, BloomModel, BloomWaveCounters, FILTER_ANNOUNCE_BYTES, FILTER_ANNOUNCE_FMP_BYTES,
    PeerRole,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamedBloomCounters {
    pub wave: BloomWaveCounters,
    pub delivered_frames: u64,
    pub transmitted_wire_bytes: u64,
    pub delivered_wire_bytes: u64,
    pub lost_wire_bytes: u64,
    pub quiescence_ns: u64,
}

impl StreamedBloomCounters {
    pub fn reconciles(&self) -> bool {
        self.wave.requested == self.wave.coalesced + self.wave.constructed
            && self.wave.constructed == self.wave.sent + self.wave.rejected
            && self.wave.message_bytes == self.wave.sent * FILTER_ANNOUNCE_BYTES
            && self.wave.fmp_bytes == self.wave.sent * FILTER_ANNOUNCE_FMP_BYTES
            && self.transmitted_wire_bytes == self.delivered_wire_bytes + self.lost_wire_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct BloomSnapshot {
    model: BloomModel,
    occupied_bits: u64,
    fpr_ppb: u64,
    estimated_cardinality: Option<u64>,
}

#[derive(Debug, Clone)]
struct PendingBloom {
    event_id: EventId,
    cause: String,
}

#[derive(Debug, Clone)]
pub(super) struct StreamedBloomRuntime {
    mode: BloomMode,
    debounce_ns: u64,
    max_fpr: f64,
    local: Vec<BloomModel>,
    peer_views: BTreeMap<(NodeId, NodeId), BloomModel>,
    pending: BTreeMap<(NodeId, NodeId), PendingBloom>,
    last_sent_ns: BTreeMap<(NodeId, NodeId), u64>,
    pub counters: StreamedBloomCounters,
}

impl StreamedBloomRuntime {
    pub(super) fn from_plan(
        plan: &NormalizedPlan,
        graph: &GraphStore,
    ) -> Result<Option<Self>, RunError> {
        let markers = plan
            .campaign
            .pointer("/instrumentation/quiescence_markers")
            .and_then(Value::as_array);
        let bloom =
            markers.is_some_and(|items| items.iter().any(|item| item.as_str() == Some("bloom")));
        let lookup =
            markers.is_some_and(|items| items.iter().any(|item| item.as_str() == Some("lookup")));
        if !bloom || (lookup && !GraphRecoveryRuntime::requested(plan)) {
            return Ok(None);
        }
        let mode = match plan
            .campaign
            .pointer("/fidelity/bloom")
            .and_then(Value::as_str)
            .unwrap_or("exact-bits")
        {
            "exact-bits" => BloomMode::ExactBits,
            "sparse-bits" => BloomMode::SparseBits,
            "occupancy" => BloomMode::Occupancy,
            other => return Err(RunError::Unsupported(format!("Bloom mode {other}"))),
        };
        let debounce_ns = duration_ns(&plan.campaign, "/protocol/parameters/bloom_update_debounce")
            .unwrap_or(500_000_000);
        let max_fpr = plan
            .campaign
            .pointer("/protocol/parameters/bloom_max_fpr")
            .and_then(Value::as_f64)
            .or_else(|| {
                plan.campaign
                    .pointer("/protocol/parameters/bloom_max_fpr_ppm")
                    .and_then(Value::as_u64)
                    .map(|value| value as f64 / 1_000_000.0)
            })
            .unwrap_or(0.20);
        let local = graph
            .node_ids()
            .map(|node| local_filter(mode, graph.address(node).map(|value| value.0)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut counters = StreamedBloomCounters::default();
        counters.wave.bitwise_operations = u64::try_from(graph.node_count())
            .map_err(|_| RunError::Arithmetic)?
            .saturating_mul(5);
        Ok(Some(Self {
            mode,
            debounce_ns,
            max_fpr,
            local,
            peer_views: BTreeMap::new(),
            pending: BTreeMap::new(),
            last_sent_ns: BTreeMap::new(),
            counters,
        }))
    }

    pub(super) fn mode(&self) -> BloomMode {
        self.mode
    }
    pub(super) fn pending(&self) -> usize {
        self.pending.len()
    }
}

impl Simulation {
    pub(super) fn request_bloom_all(
        &mut self,
        from: NodeId,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        self.request_bloom_except(from, None, cause, parent)
    }

    fn request_bloom_except(
        &mut self,
        from: NodeId,
        excluded: Option<NodeId>,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        if self.bloom.is_none() || !self.graph.is_active(from) {
            return Ok(());
        }
        for to in self.graph.active_neighbors(from) {
            if Some(to) != excluded {
                self.request_bloom(from, to, cause, parent)?;
            }
        }
        Ok(())
    }

    fn request_bloom(
        &mut self,
        from: NodeId,
        to: NodeId,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        let (due, previous_cause) = {
            let runtime = self.bloom.as_mut().unwrap();
            runtime.counters.wave.requested += 1;
            runtime.counters.wave.recomputations += 1;
            let due =
                runtime
                    .last_sent_ns
                    .get(&(from, to))
                    .map_or(self.scheduler.now_ns(), |last| {
                        self.scheduler
                            .now_ns()
                            .max(last.saturating_add(runtime.debounce_ns))
                    });
            let previous = runtime
                .pending
                .get(&(from, to))
                .map(|item| item.cause.clone());
            if previous.is_some() {
                runtime.counters.wave.coalesced += 1;
            }
            (due, previous)
        };
        if let Some(previous) = previous_cause {
            self.add_ledger(&previous, "superseded", 1, "bloom-filter");
            self.add_ledger(cause, "coalesced", 1, "bloom-filter");
        }
        let event_id = self.scheduler.schedule_coalesced(
            format!("bloom:{from}:{to}"),
            due,
            parent,
            SimEvent::BloomDue {
                from,
                to,
                cause: cause.to_owned(),
            },
        )?;
        self.bloom.as_mut().unwrap().pending.insert(
            (from, to),
            PendingBloom {
                event_id,
                cause: cause.to_owned(),
            },
        );
        self.add_ledger(cause, "requested", 1, "bloom-filter");
        Ok(())
    }

    pub(super) fn reset_local_bloom(&mut self, node: NodeId) -> Result<(), RunError> {
        let Some(runtime) = &mut self.bloom else {
            return Ok(());
        };
        runtime.local[node as usize] = local_filter(runtime.mode, Ok(self.graph.address(node)?.0))?;
        runtime.counters.wave.bitwise_operations += 5;
        runtime
            .peer_views
            .retain(|(receiver, sender), _| *receiver != node && *sender != node);
        Ok(())
    }

    pub(super) fn remove_bloom_node(&mut self, node: NodeId) {
        if let Some(runtime) = &mut self.bloom {
            runtime
                .peer_views
                .retain(|(receiver, sender), _| *receiver != node && *sender != node);
        }
    }

    pub(super) fn disconnect_bloom_edge(&mut self, left: NodeId, right: NodeId) {
        if let Some(runtime) = &mut self.bloom {
            runtime.peer_views.remove(&(left, right));
            runtime.peer_views.remove(&(right, left));
        }
    }
}

fn local_filter(
    mode: BloomMode,
    address: Result<[u8; 16], GraphError>,
) -> Result<BloomModel, RunError> {
    let mut model = BloomModel::new(mode);
    model.insert(&address?);
    Ok(model)
}

fn duration_ns(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer)?.get("nanoseconds")?.as_u64()
}

#[path = "engine_bloom_events.rs"]
mod events;

#[cfg(test)]
#[path = "engine_bloom_tests.rs"]
mod tests;
