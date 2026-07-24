use super::*;

pub(super) fn generate(config: &TrafficConfig, plan: &mut TrafficPlan) -> Result<(), TrafficError> {
    let mut ordinal = 0_u64;
    for transfer in &config.transfers {
        let chunks = transfer
            .total_bytes
            .div_ceil(transfer.visualization_chunk_bytes);
        let chunk_count = u32::try_from(chunks).map_err(|_| TrafficError::FlowCountOverflow)?;
        for chunk_index in 0..chunk_count {
            let byte_start =
                u64::from(chunk_index).saturating_mul(transfer.visualization_chunk_bytes);
            let byte_end = transfer
                .total_bytes
                .min(byte_start.saturating_add(transfer.visualization_chunk_bytes));
            let session_action = if chunk_index == 0 {
                SessionAction::Setup
            } else if chunk_index + 1 == chunk_count {
                SessionAction::Teardown
            } else {
                SessionAction::Reuse
            };
            let rate_offset_ns = u64::try_from(
                u128::from(byte_start).saturating_mul(8_000_000_000)
                    / u128::from(config.rate_bps.max(1)),
            )
            .unwrap_or(u64::MAX);
            push_flow(
                plan,
                Flow {
                    id: format!("{}-chunk-{chunk_index:08}", transfer.id),
                    source: transfer.source,
                    destination: transfer.destination,
                    offered_at_ns: transfer.start_ns.saturating_add(rate_offset_ns),
                    useful_payload_bytes: byte_end - byte_start,
                    session_action,
                    shape: FlowShape::ApplicationTransfer {
                        transfer_id: transfer.id.clone(),
                        chunk_index,
                        chunk_count,
                        total_bytes: transfer.total_bytes,
                        byte_start,
                        byte_end,
                    },
                },
                ordinal,
            )?;
            ordinal = ordinal
                .checked_add(1)
                .ok_or(TrafficError::FlowCountOverflow)?;
        }
    }
    plan.flows.sort_by(|left, right| {
        (left.offered_at_ns, &left.id).cmp(&(right.offered_at_ns, &right.id))
    });
    Ok(())
}
