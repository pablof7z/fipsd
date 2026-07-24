use super::*;
use crate::{TrafficConfig, TrafficModel, TrafficPlan, TransferSpec};

const MAX_ROUTED_FLOWS: u64 = 100_000;

impl RoutedTrafficRuntime {
    pub(crate) fn from_plan(plan: &NormalizedPlan, nodes: u32) -> Result<Option<Self>, RunError> {
        if RecoveryEngine::protocol_requested(plan) && !GraphRecoveryRuntime::requested(plan) {
            return Ok(None);
        }
        let model =
            TrafficModel::parse(optional_str(&plan.campaign, "/traffic/model")?.unwrap_or("idle"))
                .map_err(|error| RunError::Unsupported(error.to_string()))?;
        if model == TrafficModel::Idle {
            return Ok(None);
        }
        let payload_bytes = optional_u64(&plan.campaign, "/traffic/payload_bytes")?.unwrap_or(512);
        let rate_bps = optional_u64(&plan.campaign, "/traffic/rate_bps")?.unwrap_or(4_096_000);
        let flow_count =
            optional_u64(&plan.campaign, "/traffic/parameters/flow_count")?.unwrap_or(128);
        let segments_per_stream =
            optional_u64(&plan.campaign, "/traffic/parameters/segments_per_stream")?.unwrap_or(32);
        let burst_size =
            optional_u64(&plan.campaign, "/traffic/parameters/burst_size")?.unwrap_or(16);
        let burst_interval_ns =
            optional_u64(&plan.campaign, "/traffic/parameters/burst_interval_ns")?
                .unwrap_or(250_000_000);
        let transfers = transfer_specs(plan)?;
        let count = match model {
            TrafficModel::AllToAll => u64::from(nodes) * u64::from(nodes.saturating_sub(1)),
            TrafficModel::PersistentStreams => flow_count
                .checked_mul(segments_per_stream)
                .ok_or(RunError::Arithmetic)?,
            TrafficModel::ExplicitTransfers => {
                transfers.iter().try_fold(0_u64, |total, transfer| {
                    total
                        .checked_add(
                            transfer
                                .total_bytes
                                .div_ceil(transfer.visualization_chunk_bytes),
                        )
                        .ok_or(RunError::Arithmetic)
                })?
            }
            _ => flow_count,
        };
        if count > MAX_ROUTED_FLOWS {
            return Err(RunError::Unsupported(format!(
                "routed individual traffic has {count} flows; limit is {MAX_ROUTED_FLOWS}, use cohort fidelity above it"
            )));
        }
        let interval_ns = payload_bytes
            .saturating_mul(8)
            .saturating_mul(1_000_000_000)
            .checked_div(rate_bps.max(1))
            .unwrap_or(1)
            .max(1);
        let traffic = TrafficPlan::generate(&TrafficConfig {
            model,
            nodes,
            flow_count,
            payload_bytes,
            rate_bps,
            interval_ns,
            segments_per_stream: u32::try_from(segments_per_stream)
                .map_err(|_| RunError::Unsupported("segments_per_stream exceeds u32".to_owned()))?,
            burst_size: u32::try_from(burst_size)
                .map_err(|_| RunError::Unsupported("burst_size exceeds u32".to_owned()))?,
            burst_interval_ns,
            seed: plan.seed,
            transfers,
        })
        .map_err(|error| RunError::Unsupported(error.to_string()))?;
        Ok(Some(Self {
            plan: traffic,
            start_ns: if model == TrafficModel::ExplicitTransfers {
                0
            } else {
                optional_u64(&plan.campaign, "/traffic/parameters/start_ns")?.unwrap_or(500_000_000)
            },
            counters: RoutedTrafficCounters::default(),
            last_useful_delivery_ns: None,
        }))
    }
}

fn transfer_specs(plan: &NormalizedPlan) -> Result<Vec<TransferSpec>, RunError> {
    let Some(values) = plan
        .campaign
        .pointer("/traffic/transfers")
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("application-transfer")
                .to_owned();
            let source = transfer_node(value, "source", index)?;
            let destination = transfer_node(value, "destination", index)?;
            let total_bytes = transfer_u64(value, "total_bytes", index)?;
            let visualization_chunk_bytes = value
                .get("visualization_chunk_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(1_000_000);
            let start_ns = value
                .get("start")
                .and_then(|start| start.get("nanoseconds"))
                .and_then(Value::as_u64)
                .unwrap_or(500_000_000);
            Ok(TransferSpec {
                id,
                source,
                destination,
                total_bytes,
                visualization_chunk_bytes,
                start_ns,
            })
        })
        .collect()
}

fn transfer_node(value: &Value, key: &str, index: usize) -> Result<NodeId, RunError> {
    let number = transfer_u64(value, key, index)?;
    NodeId::try_from(number)
        .map_err(|_| RunError::Unsupported(format!("traffic transfer {index} {key} exceeds u32")))
}

fn transfer_u64(value: &Value, key: &str, index: usize) -> Result<u64, RunError> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| {
        RunError::Unsupported(format!("traffic transfer {index} requires integer {key}"))
    })
}
