use super::*;

impl RunConfig {
    pub(super) fn from_plan(plan: &NormalizedPlan) -> Result<Self, RunError> {
        let campaign = &plan.campaign;
        let mode = scalar_str(campaign, "/engine/modes")?;
        if mode != "compact-discrete-event" {
            return Err(RunError::Unsupported(format!(
                "/engine/modes must be compact-discrete-event, got {mode}"
            )));
        }
        let nodes = scalar_u64(campaign, "/scale/nodes")?;
        if !(2..=2_000_000).contains(&nodes) {
            return Err(RunError::Unsupported(format!(
                "individual engine node count {nodes} is outside 2..=2000000"
            )));
        }
        let arrivals = optional_u64(campaign, "/identities/arrivals/count")?.unwrap_or(0);
        if arrivals >= nodes {
            return Err(RunError::Unsupported(format!(
                "arrival count {arrivals} must be less than node count {nodes}"
            )));
        }
        let topology = TopologyKind::parse(scalar_str(campaign, "/topology/generator")?)?;
        let average_degree = optional_u64(campaign, "/topology/average_degree")?.unwrap_or(2);
        let explicit_edges = campaign
            .pointer("/topology/explicit_edges")
            .and_then(Value::as_array)
            .map(|edges| {
                edges
                    .iter()
                    .map(|edge| {
                        let pair = edge.as_array().ok_or_else(|| {
                            RunError::Unsupported("explicit edge must be an array".to_owned())
                        })?;
                        Ok((
                            pair[0].as_u64().unwrap_or(u64::MAX) as NodeId,
                            pair[1].as_u64().unwrap_or(u64::MAX) as NodeId,
                        ))
                    })
                    .collect::<Result<Vec<_>, RunError>>()
            })
            .transpose()?
            .unwrap_or_default();
        let attachment = optional_str(campaign, "/identities/arrivals/attachment")?
            .map(AttachmentSelector::parse)
            .transpose()?
            .unwrap_or(AttachmentSelector::CurrentRoot);
        let address_policy = optional_str(campaign, "/identities/arrivals/address_policy")?
            .unwrap_or("strictly-lower-than-current-root")
            .to_owned();
        if !matches!(
            address_policy.as_str(),
            "one-lower"
                | "one-lower-than-current-root"
                | "strictly-descending"
                | "strictly-lower-than-current-root"
                | "precomputed-ladder"
        ) {
            return Err(RunError::Unsupported(format!(
                "unsupported identity address policy {address_policy}"
            )));
        }
        let precomputed_ladder = campaign
            .pointer("/identities/arrivals/precomputed_ladder")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or_else(|| {
                                RunError::Unsupported(
                                    "precomputed ladder addresses must be strings".to_owned(),
                                )
                            })
                            .and_then(|value| NodeAddress::from_hex(value).map_err(Into::into))
                    })
                    .collect::<Result<Vec<_>, RunError>>()
            })
            .transpose()?
            .unwrap_or_default();
        if address_policy == "precomputed-ladder" && precomputed_ladder.len() < arrivals as usize {
            return Err(RunError::Unsupported(format!(
                "precomputed ladder has {} addresses for {arrivals} arrivals",
                precomputed_ladder.len()
            )));
        }
        let attacker_budget_mode =
            optional_str(campaign, "/identities/arrivals/attacker_budget/mode")?
                .unwrap_or("free-input")
                .to_owned();
        let attacker_operations =
            optional_u64(campaign, "/identities/arrivals/attacker_budget/operations")?;
        let arrival_start_ns = optional_duration(campaign, "/identities/arrivals/schedule/start")?
            .unwrap_or(2_000_000_000);
        let arrival_interval_ns =
            optional_duration(campaign, "/identities/arrivals/schedule/interval")?
                .unwrap_or(500_000_000);
        if arrival_interval_ns > 10_000_000_000 {
            return Err(RunError::Unsupported(
                "arrival cadence must be between simultaneous and 10s".to_owned(),
            ));
        }
        let debounce_ns =
            optional_duration(campaign, "/protocol/parameters/tree_announce_debounce")?
                .unwrap_or(DEFAULT_DEBOUNCE_NS);
        let parent_hysteresis_ppm =
            optional_u64(campaign, "/protocol/parameters/parent_hysteresis_ppm")?.unwrap_or(0)
                as u32;
        let parent_hold_down_ns =
            optional_duration(campaign, "/protocol/parameters/parent_hold_down")?.unwrap_or(0);
        let ordering = match optional_str(campaign, "/links/ordering")?.unwrap_or("datagram") {
            "datagram" => LinkOrdering::Datagram,
            "stream" => LinkOrdering::Stream,
            other => {
                return Err(RunError::Unsupported(format!(
                    "unsupported link ordering {other}"
                )));
            }
        };
        let transport = scalar_str(campaign, "/transports/assignment")?;
        let transport_overhead_bytes = match transport {
            "all-udp" => 28,
            "all-tcp" => 40,
            "all-ethernet" => 18,
            other => {
                return Err(RunError::Unsupported(format!(
                    "M1 individual engine requires a homogeneous transport, got {other}"
                )));
            }
        };
        let link = LinkConfig {
            latency_ns: optional_duration(campaign, "/links/latency")?.unwrap_or(1_000_000),
            bandwidth_bps: optional_u64(campaign, "/links/bandwidth_bps")?.unwrap_or(1_000_000_000),
            loss_ppm: optional_u64(campaign, "/links/loss_ppm")?.unwrap_or(0) as u32,
            duplication_ppm: optional_u64(campaign, "/links/duplication_ppm")?.unwrap_or(0) as u32,
            ordering,
            mtu_bytes: optional_u64(campaign, "/links/mtu_bytes")?.unwrap_or(9_000),
            queue_bytes: optional_u64(campaign, "/links/queue_bytes")?.unwrap_or(1_048_576),
            transport_overhead_bytes,
        };
        let inject_parent_loop_event = campaign
            .pointer("/events")
            .and_then(Value::as_array)
            .and_then(|events| {
                events.iter().find(|event| {
                    event.get("action").and_then(Value::as_str) == Some("inject-parent-loop")
                })
            });
        let inject_parent_loop_at_ns = match inject_parent_loop_event {
            Some(event) => duration_value(event.get("at"))?,
            None => None,
        };
        let lifecycle = campaign
            .pointer("/events")
            .and_then(Value::as_array)
            .map(|events| {
                events
                    .iter()
                    .filter_map(|event| {
                        let action = event.get("action").and_then(Value::as_str)?;
                        let reappear = match action {
                            "disappear-node" => false,
                            "reappear-node" => true,
                            _ => return None,
                        };
                        Some((event, reappear))
                    })
                    .map(|(event, reappear)| {
                        let at_ns = duration_value(event.get("at"))?.ok_or_else(|| {
                            RunError::Unsupported(
                                "disappear/reappear event requires an at duration".to_owned(),
                            )
                        })?;
                        let node = event
                            .get("target")
                            .and_then(Value::as_u64)
                            .or_else(|| event.pointer("/parameters/node").and_then(Value::as_u64))
                            .ok_or_else(|| {
                                RunError::Unsupported(
                                    "disappear/reappear event requires an integer target"
                                        .to_owned(),
                                )
                            })?;
                        if node >= nodes {
                            return Err(RunError::Unsupported(format!(
                                "lifecycle event targets node {node}, but scale is {nodes}"
                            )));
                        }
                        Ok(LifecycleInput {
                            at_ns,
                            node: node as NodeId,
                            reappear,
                        })
                    })
                    .collect::<Result<Vec<_>, RunError>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            nodes: nodes as u32,
            arrivals: arrivals as u32,
            topology,
            average_degree: average_degree as u32,
            explicit_edges,
            attachment,
            address_policy,
            precomputed_ladder,
            attacker_budget_mode,
            attacker_operations,
            arrival_start_ns,
            arrival_interval_ns,
            debounce_ns,
            parent_hysteresis_ppm,
            parent_hold_down_ns,
            link,
            inject_parent_loop_at_ns,
            lifecycle,
        })
    }
}
