use super::*;

#[path = "traffic_explicit.rs"]
mod explicit;

pub(super) fn generate(config: &TrafficConfig) -> Result<TrafficPlan, TrafficError> {
    validate(config)?;
    let mut plan = TrafficPlan::default();
    match config.model {
        TrafficModel::PersistentStreams => persistent_streams(config, &mut plan)?,
        TrafficModel::ExplicitTransfers => explicit::generate(config, &mut plan)?,
        _ => independent_flows(config, &mut plan)?,
    }
    finish(plan)
}

fn validate(config: &TrafficConfig) -> Result<(), TrafficError> {
    if config.nodes < 2 && config.model != TrafficModel::Idle {
        return Err(TrafficError::TooFewNodes(config.nodes));
    }
    if config.payload_bytes == 0 && config.model != TrafficModel::Idle {
        return Err(TrafficError::ZeroPayload);
    }
    for transfer in &config.transfers {
        if transfer.total_bytes == 0 || transfer.visualization_chunk_bytes == 0 {
            return Err(TrafficError::ZeroPayload);
        }
        for node in [transfer.source, transfer.destination] {
            if node >= config.nodes {
                return Err(TrafficError::InvalidTransferEndpoint {
                    id: transfer.id.clone(),
                    node,
                    nodes: config.nodes,
                });
            }
        }
    }
    if config.model == TrafficModel::PersistentStreams && config.segments_per_stream == 0 {
        return Err(TrafficError::ZeroSegments);
    }
    if config.model == TrafficModel::Bursty && config.burst_size == 0 {
        return Err(TrafficError::ZeroBurstSize);
    }
    if config.model == TrafficModel::Bursty && config.burst_interval_ns == 0 {
        return Err(TrafficError::ZeroBurstInterval);
    }
    Ok(())
}

fn independent_flows(config: &TrafficConfig, plan: &mut TrafficPlan) -> Result<(), TrafficError> {
    let count = match config.model {
        TrafficModel::Idle => 0,
        TrafficModel::AllToAll => {
            u64::from(config.nodes) * u64::from(config.nodes.saturating_sub(1))
        }
        _ => config.flow_count,
    };
    for ordinal in 0..count {
        let shape = if config.model == TrafficModel::Bursty {
            let burst_size = u64::from(config.burst_size);
            let member_count = (count - ordinal / burst_size * burst_size).min(burst_size);
            FlowShape::BurstMember {
                burst_index: ordinal / burst_size,
                member_index: (ordinal % burst_size) as u32,
                member_count: member_count as u32,
            }
        } else {
            FlowShape::Single
        };
        let offered_at_ns = if config.model == TrafficModel::Bursty {
            (ordinal / u64::from(config.burst_size)).saturating_mul(config.burst_interval_ns)
        } else {
            ordinal.saturating_mul(config.interval_ns)
        };
        let (source, destination) = endpoints(config, ordinal);
        let session_action = standard_session_action(config.model, ordinal);
        let useful_payload_bytes = payload_bytes(config, ordinal);
        push_flow(
            plan,
            Flow {
                id: format!("flow-{ordinal:08}"),
                source,
                destination,
                offered_at_ns,
                useful_payload_bytes,
                session_action,
                shape,
            },
            ordinal,
        )?;
    }
    Ok(())
}

fn persistent_streams(config: &TrafficConfig, plan: &mut TrafficPlan) -> Result<(), TrafficError> {
    let segments = u64::from(config.segments_per_stream);
    config
        .flow_count
        .checked_mul(segments)
        .ok_or(TrafficError::FlowCountOverflow)?;
    for stream in 0..config.flow_count {
        let (source, destination) = endpoints(config, stream);
        for segment in 0..config.segments_per_stream {
            let ordinal = stream
                .checked_mul(segments)
                .and_then(|value| value.checked_add(u64::from(segment)))
                .ok_or(TrafficError::FlowCountOverflow)?;
            let emission = u64::from(segment)
                .checked_mul(config.flow_count)
                .and_then(|value| value.checked_add(stream))
                .ok_or(TrafficError::FlowCountOverflow)?;
            let session_action = if segment == 0 {
                SessionAction::Setup
            } else if segment + 1 == config.segments_per_stream {
                SessionAction::Teardown
            } else {
                SessionAction::Reuse
            };
            push_flow(
                plan,
                Flow {
                    id: format!("stream-{stream:08}-segment-{segment:06}"),
                    source,
                    destination,
                    offered_at_ns: emission.saturating_mul(config.interval_ns),
                    useful_payload_bytes: config.payload_bytes,
                    session_action,
                    shape: FlowShape::StreamSegment {
                        stream_id: format!("stream-{stream:08}"),
                        segment_index: segment,
                        segment_count: config.segments_per_stream,
                    },
                },
                ordinal,
            )?;
        }
    }
    plan.flows.sort_by(|left, right| {
        left.offered_at_ns
            .cmp(&right.offered_at_ns)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(())
}

fn push_flow(plan: &mut TrafficPlan, flow: Flow, ordinal: u64) -> Result<(), TrafficError> {
    if flow.source == flow.destination {
        return Err(TrafficError::SelfFlow {
            ordinal,
            node: flow.source,
        });
    }
    match flow.session_action {
        SessionAction::Setup => plan.session_setups += 1,
        SessionAction::Teardown => plan.session_teardowns += 1,
        SessionAction::Rekey => plan.rekeys += 1,
        SessionAction::Reuse => {}
    }
    plan.offered_useful_bytes = plan
        .offered_useful_bytes
        .saturating_add(flow.useful_payload_bytes);
    plan.flows.push(flow);
    Ok(())
}

fn finish(mut plan: TrafficPlan) -> Result<TrafficPlan, TrafficError> {
    plan.setup_message_bytes = plan
        .session_setups
        .saturating_mul(SESSION_SETUP_MESSAGE_BYTES + SESSION_ACK_MESSAGE_BYTES);
    let projected = plan
        .flows
        .iter()
        .map(|flow| flow.useful_payload_bytes)
        .sum::<u64>();
    if projected != plan.offered_useful_bytes {
        return Err(TrafficError::OfferedLoadDrift {
            recorded: plan.offered_useful_bytes,
            projected,
        });
    }
    Ok(plan)
}

fn standard_session_action(model: TrafficModel, ordinal: u64) -> SessionAction {
    match model {
        TrafficModel::SessionChurn if ordinal % 2 == 0 => SessionAction::Setup,
        TrafficModel::SessionChurn => SessionAction::Teardown,
        _ if ordinal % 17 == 0 => SessionAction::Setup,
        _ if ordinal % 101 == 0 => SessionAction::Rekey,
        _ => SessionAction::Reuse,
    }
}

fn payload_bytes(config: &TrafficConfig, ordinal: u64) -> u64 {
    match config.model {
        TrafficModel::ElephantsAndMice if ordinal % 10 == 0 => {
            config.payload_bytes.saturating_mul(100)
        }
        TrafficModel::PayloadSweep => {
            const SIZES: [u64; 8] = [64, 256, 1024, 1200, 1279, 1280, 1500, 9000];
            SIZES[ordinal as usize % SIZES.len()]
        }
        _ => config.payload_bytes,
    }
}

fn endpoints(config: &TrafficConfig, ordinal: u64) -> (u32, u32) {
    let nodes = u64::from(config.nodes);
    match config.model {
        TrafficModel::Idle | TrafficModel::ExplicitTransfers => (0, 0),
        TrafficModel::UniformRandom | TrafficModel::PersistentStreams | TrafficModel::Bursty => {
            let source = draw(config.seed, ordinal, 0) % nodes;
            let offset = 1 + draw(config.seed, ordinal, 1) % (nodes - 1);
            (source as u32, ((source + offset) % nodes) as u32)
        }
        TrafficModel::Permutation => {
            let source = ordinal % nodes;
            (source as u32, ((source + 1) % nodes) as u32)
        }
        TrafficModel::AllToAll => {
            let source = ordinal / (nodes - 1);
            let mut destination = ordinal % (nodes - 1);
            if destination >= source {
                destination += 1;
            }
            (source as u32, destination as u32)
        }
        TrafficModel::Zipf => {
            let source = ordinal % nodes;
            let sample = draw(config.seed, ordinal, 2) as f64 / u64::MAX as f64;
            let mut destination = (sample * sample * nodes as f64) as u64 % nodes;
            if destination == source {
                destination = (destination + 1) % nodes;
            }
            (source as u32, destination as u32)
        }
        TrafficModel::Incast => ((ordinal % (nodes - 1) + 1) as u32, 0),
        TrafficModel::Outcast => (0, (ordinal % (nodes - 1) + 1) as u32),
        TrafficModel::ElephantsAndMice | TrafficModel::PayloadSweep => {
            let source = ordinal % nodes;
            (
                source as u32,
                ((source + nodes / 2).max(source + 1) % nodes) as u32,
            )
        }
        TrafficModel::CrossCut => {
            let half = (nodes / 2).max(1);
            let source = ordinal % half;
            let destination = half + ordinal % (nodes - half);
            (source as u32, destination as u32)
        }
        TrafficModel::SessionChurn => {
            let pair = (ordinal / 2) % nodes;
            (pair as u32, ((pair + 1) % nodes) as u32)
        }
    }
}

fn draw(seed: u64, ordinal: u64, lane: u64) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(lane.to_le_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[0..8].try_into().expect("slice length"))
}
