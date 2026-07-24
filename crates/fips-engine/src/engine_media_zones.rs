use super::*;

#[derive(Debug, Clone)]
struct MediaZone {
    id: String,
    nodes: BTreeSet<NodeId>,
    bandwidth_bps: u64,
    latency_ns: u64,
    loss_ppm: u32,
    mtu_bytes: u64,
    queue_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct MediaZonePlan {
    zones: Vec<MediaZone>,
    by_node: Vec<Option<usize>>,
}

impl MediaZonePlan {
    pub(super) fn from_plan(plan: &NormalizedPlan, nodes: u32) -> Result<Self, RunError> {
        let Some(values) = plan
            .campaign
            .pointer("/topology/media_zones")
            .and_then(Value::as_array)
        else {
            return Ok(Self {
                zones: vec![],
                by_node: vec![None; nodes as usize],
            });
        };
        let mut plan = Self {
            zones: Vec::new(),
            by_node: vec![None; nodes as usize],
        };
        for (index, value) in values.iter().enumerate() {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| unsupported("media zone requires id"))?
                .to_owned();
            let members = value
                .get("nodes")
                .and_then(Value::as_array)
                .ok_or_else(|| unsupported("media zone requires nodes"))?;
            let mut zone_nodes = BTreeSet::new();
            for member in members {
                let node = member
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|node| *node < nodes)
                    .ok_or_else(|| unsupported("media zone node is outside scale"))?;
                if plan.by_node[node as usize].replace(index).is_some() {
                    return Err(unsupported(
                        "a node cannot belong to multiple shared media zones",
                    ));
                }
                zone_nodes.insert(node);
            }
            plan.zones.push(MediaZone {
                id,
                nodes: zone_nodes,
                bandwidth_bps: positive(value, "bandwidth_bps")?,
                latency_ns: duration(value.get("latency"))?,
                loss_ppm: bounded_u32(value, "loss_ppm", 1_000_000)?,
                mtu_bytes: positive(value, "mtu_bytes")?,
                queue_bytes: positive(value, "queue_bytes")?,
            });
        }
        Ok(plan)
    }

    pub(super) fn configure_edge(
        &self,
        edge: EdgeId,
        from: NodeId,
        to: NodeId,
        transports: &TransportPlan,
        links: &mut LinkService,
    ) -> Result<(), RunError> {
        let mut config = transports.link_config(from, to);
        let zone =
            self.by_node[from as usize].filter(|zone| Some(*zone) == self.by_node[to as usize]);
        if let Some(index) = zone {
            let medium = &self.zones[index];
            debug_assert!(medium.nodes.contains(&from) && medium.nodes.contains(&to));
            config.bandwidth_bps = config.bandwidth_bps.min(medium.bandwidth_bps);
            config.latency_ns = config.latency_ns.saturating_add(medium.latency_ns);
            config.loss_ppm = combine_loss(config.loss_ppm, medium.loss_ppm);
            config.mtu_bytes = config.mtu_bytes.min(medium.mtu_bytes);
            config.queue_bytes = config.queue_bytes.min(medium.queue_bytes);
            links.set_config(edge, config)?;
            links.set_shared_group(edge, index as u32)?;
        } else {
            links.set_config(edge, config)?;
        }
        Ok(())
    }

    pub(super) fn zone_id(&self, node: NodeId) -> Option<&str> {
        self.by_node
            .get(node as usize)?
            .and_then(|index| self.zones.get(index).map(|zone| zone.id.as_str()))
    }
}

fn positive(value: &Value, field: &str) -> Result<u64, RunError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| unsupported(&format!("media zone {field} must be positive")))
}

fn bounded_u32(value: &Value, field: &str, maximum: u64) -> Result<u32, RunError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value <= maximum)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| unsupported(&format!("invalid media zone {field}")))
}

fn duration(value: Option<&Value>) -> Result<u64, RunError> {
    value
        .and_then(Value::as_object)
        .and_then(|value| value.get("nanoseconds"))
        .and_then(Value::as_u64)
        .ok_or_else(|| unsupported("media zone latency must be a normalized duration"))
}

fn combine_loss(left: u32, right: u32) -> u32 {
    let survive = u64::from(1_000_000 - left) * u64::from(1_000_000 - right) / 1_000_000;
    (1_000_000 - survive) as u32
}

fn unsupported(message: &str) -> RunError {
    RunError::Unsupported(message.to_owned())
}
