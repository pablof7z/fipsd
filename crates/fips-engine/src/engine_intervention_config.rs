use super::*;

pub(super) struct InterventionPlan {
    pub manual_arrivals: Vec<ManualArrivalInput>,
    pub cuts: Vec<NetworkCutInput>,
    pub link_updates: Vec<LinkUpdateInput>,
    pub rekeys: Vec<SessionRekeyInput>,
    pub cache_expiries: Vec<CacheExpiryInput>,
    pub lookup_waves: Vec<LookupWaveInput>,
    pub transport_classes: Vec<TransportClassInput>,
    pub parent_costs: Vec<ParentCostInput>,
    pub sybils: Vec<SybilArrivalInput>,
}

const SUPPORTED_ACTIONS: &[&str] = &[
    "introduce-lower-root-identities",
    "introduce-lower-root-node",
    "introduce-node",
    "inject-parent-loop",
    "disappear-node",
    "reappear-node",
    "partition-network",
    "merge-network",
    "set-link-conditions",
    "restore-link-conditions",
    "synchronized-session-rekey",
    "expire-coordinate-cache",
    "simultaneous-lookups",
    "fail-transport-class",
    "restore-transport-class",
    "swap-parent-ancestry",
    "alternate-parent-quality",
    "attach-authenticated-sybils",
];

pub(super) fn parse_interventions(
    campaign: &Value,
    nodes: u64,
) -> Result<InterventionPlan, RunError> {
    let mut plan = InterventionPlan {
        manual_arrivals: Vec::new(),
        cuts: Vec::new(),
        link_updates: Vec::new(),
        rekeys: Vec::new(),
        cache_expiries: Vec::new(),
        lookup_waves: Vec::new(),
        transport_classes: Vec::new(),
        parent_costs: Vec::new(),
        sybils: Vec::new(),
    };
    let Some(events) = campaign.pointer("/events").and_then(Value::as_array) else {
        return Ok(plan);
    };
    for (index, event) in events.iter().enumerate() {
        let action = event.get("action").and_then(Value::as_str).unwrap_or("");
        if !SUPPORTED_ACTIONS.contains(&action) {
            return Err(RunError::Unsupported(format!(
                "unsupported individual-engine event action {action} at /events/{index}"
            )));
        }
        match action {
            "introduce-lower-root-node" | "introduce-node" => {
                let mut targets = event
                    .pointer("/parameters/attachments")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| {
                                value.as_u64().ok_or_else(|| {
                                    RunError::Unsupported(format!(
                                        "{action} attachments must be node IDs"
                                    ))
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                if let Some(target) = event.get("target").and_then(Value::as_u64) {
                    targets.push(target);
                }
                targets.sort_unstable();
                targets.dedup();
                if let Some(target) = targets.iter().find(|target| **target >= nodes) {
                    return Err(RunError::Unsupported(format!(
                        "{action} target {target} is outside node count {nodes}"
                    )));
                }
                plan.manual_arrivals.push(ManualArrivalInput {
                    at_ns: required_time(event, action)?,
                    lower_root: action == "introduce-lower-root-node",
                    targets: targets.into_iter().map(|value| value as NodeId).collect(),
                });
            }
            "partition-network" | "merge-network" => {
                let group = node_group(event, nodes, action)?;
                plan.cuts.push(NetworkCutInput {
                    id: event_id(event, index, action),
                    at_ns: required_time(event, action)?,
                    nodes: group,
                    enabled: action == "merge-network",
                });
            }
            "set-link-conditions" | "restore-link-conditions" => {
                plan.link_updates
                    .push(parse_link_update(event, index, action)?);
            }
            "synchronized-session-rekey" => plan.rekeys.push(SessionRekeyInput {
                id: event_id(event, index, action),
                at_ns: required_time(event, action)?,
            }),
            "expire-coordinate-cache" => plan.cache_expiries.push(CacheExpiryInput {
                id: event_id(event, index, action),
                at_ns: required_time(event, action)?,
            }),
            "simultaneous-lookups" => {
                let count = parameter_u64(event, "count");
                if count == Some(0) || count.is_some_and(|value| value > 100_000) {
                    return Err(RunError::Unsupported(
                        "simultaneous-lookups count must be in 1..=100000".to_owned(),
                    ));
                }
                plan.lookup_waves.push(LookupWaveInput {
                    id: event_id(event, index, action),
                    at_ns: required_time(event, action)?,
                    count,
                });
            }
            "fail-transport-class" | "restore-transport-class" => {
                let profile = event
                    .get("target")
                    .and_then(Value::as_str)
                    .or_else(|| event.pointer("/parameters/profile").and_then(Value::as_str))
                    .ok_or_else(|| {
                        RunError::Unsupported(format!(
                            "{action} requires a transport profile target"
                        ))
                    })?;
                plan.transport_classes.push(TransportClassInput {
                    id: event_id(event, index, action),
                    at_ns: required_time(event, action)?,
                    profile: profile.to_owned(),
                    restore: action == "restore-transport-class",
                });
            }
            "swap-parent-ancestry" | "alternate-parent-quality" => {
                intervention_inputs::parse_parent_costs(
                    event,
                    index,
                    action,
                    nodes,
                    &mut plan.parent_costs,
                )?;
            }
            "attach-authenticated-sybils" => {
                intervention_inputs::parse_sybils(campaign, event, index, nodes, &mut plan.sybils)?;
            }
            _ => {}
        }
    }
    plan.manual_arrivals.sort_by_key(|input| input.at_ns);
    Ok(plan)
}

pub(super) fn required_time(event: &Value, action: &str) -> Result<u64, RunError> {
    duration_value(event.get("at"))?
        .ok_or_else(|| RunError::Unsupported(format!("{action} event requires an at duration")))
}

pub(super) fn event_id(event: &Value, index: usize, action: &str) -> String {
    event
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{action}-{index}"))
}

fn node_group(event: &Value, nodes: u64, action: &str) -> Result<Vec<NodeId>, RunError> {
    let mut group = event
        .pointer("/parameters/nodes")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
        .unwrap_or_default();
    if let Some(target) = event
        .get("target")
        .and_then(Value::as_u64)
        .or_else(|| event.pointer("/parameters/node").and_then(Value::as_u64))
    {
        group.push(target);
    }
    group.sort_unstable();
    group.dedup();
    if group.is_empty() || group.len() as u64 >= nodes || group.iter().any(|node| *node >= nodes) {
        return Err(RunError::Unsupported(format!(
            "{action} requires a non-empty proper group of valid node IDs"
        )));
    }
    Ok(group.into_iter().map(|node| node as NodeId).collect())
}

fn parse_link_update(
    event: &Value,
    index: usize,
    action: &str,
) -> Result<LinkUpdateInput, RunError> {
    let edge = event
        .get("target")
        .and_then(Value::as_u64)
        .or_else(|| event.pointer("/parameters/edge").and_then(Value::as_u64))
        .ok_or_else(|| RunError::Unsupported(format!("{action} requires an integer edge")))?;
    let loss = event
        .pointer("/parameters/loss_ppm")
        .and_then(Value::as_u64);
    if loss.is_some_and(|value| value > 1_000_000) {
        return Err(RunError::Unsupported("loss_ppm exceeds 1000000".to_owned()));
    }
    Ok(LinkUpdateInput {
        id: event_id(event, index, action),
        at_ns: required_time(event, action)?,
        edge: u32::try_from(edge)
            .map_err(|_| RunError::Unsupported("edge ID exceeds u32".to_owned()))?,
        restore: action == "restore-link-conditions",
        bandwidth_bps: parameter_u64(event, "bandwidth_bps"),
        latency_ns: event
            .pointer("/parameters/latency")
            .and_then(|value| value.get("nanoseconds"))
            .and_then(Value::as_u64),
        jitter_ns: event
            .pointer("/parameters/jitter")
            .and_then(|value| value.get("nanoseconds"))
            .and_then(Value::as_u64),
        loss_ppm: loss.map(|value| value as u32),
        mtu_bytes: parameter_u64(event, "mtu_bytes"),
        queue_bytes: parameter_u64(event, "queue_bytes"),
    })
}

pub(super) fn parameter_u64(event: &Value, name: &str) -> Option<u64> {
    event
        .pointer(&format!("/parameters/{name}"))
        .and_then(Value::as_u64)
}
