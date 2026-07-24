use super::*;
use fips_transport::{
    MediaKind, MediaOrdering, MediaProfile, builtin_profile, deterministic_weighted_index,
};

#[derive(Debug, Clone)]
pub(super) struct NodeTransportProfile {
    pub name: String,
    pub media: MediaProfile,
    pub loss_ppm: u32,
    pub queue_bytes: u64,
    pub weight: u64,
}

#[derive(Debug, Clone)]
pub(super) struct TransportPlan {
    profiles: Vec<NodeTransportProfile>,
    assignments: Vec<usize>,
    base: LinkConfig,
    mixed: bool,
}

impl TransportPlan {
    pub(super) fn from_campaign(
        campaign: &Value,
        nodes: u32,
        seed: u64,
        base: LinkConfig,
    ) -> Result<Self, RunError> {
        let assignment = scalar_str(campaign, "/transports/assignment")?;
        let (profiles, mixed) = match assignment {
            "all-udp" => (vec![homogeneous("udp", MediaKind::Udp, &base)], false),
            "all-tcp" => (vec![homogeneous("tcp", MediaKind::Tcp, &base)], false),
            "all-ethernet" => (
                vec![homogeneous("ethernet", MediaKind::Ethernet, &base)],
                false,
            ),
            "random-mixed" => (parse_profiles(campaign, &base)?, true),
            other => {
                return Err(RunError::Unsupported(format!(
                    "unsupported individual-engine transport assignment {other}"
                )));
            }
        };
        let weights = profiles
            .iter()
            .map(|profile| profile.weight)
            .collect::<Vec<_>>();
        let assignments = (0..nodes)
            .map(|node| {
                if mixed {
                    deterministic_weighted_index(seed, u64::from(node), &weights).ok_or_else(|| {
                        RunError::Unsupported("transport profile weights sum to zero".to_owned())
                    })
                } else {
                    Ok(0)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            profiles,
            assignments,
            base,
            mixed,
        })
    }

    pub(super) fn profile(&self, node: NodeId) -> &NodeTransportProfile {
        &self.profiles[self.assignments[node as usize]]
    }

    pub(super) fn is_mixed(&self) -> bool {
        self.mixed
    }

    pub(super) fn contains_profile(&self, name: &str) -> bool {
        self.profiles.iter().any(|profile| profile.name == name)
    }

    pub(super) fn profile_counts(&self) -> BTreeMap<String, u64> {
        self.assignments
            .iter()
            .fold(BTreeMap::new(), |mut counts, index| {
                *counts
                    .entry(self.profiles[*index].name.clone())
                    .or_default() += 1;
                counts
            })
    }

    pub(super) fn link_config(&self, from: NodeId, to: NodeId) -> LinkConfig {
        if !self.mixed {
            return self.base.clone();
        }
        let left = self.profile(from);
        let right = self.profile(to);
        LinkConfig {
            latency_ns: self
                .base
                .latency_ns
                .saturating_add(left.media.latency_ns)
                .saturating_add(right.media.latency_ns),
            jitter_ns: self
                .base
                .jitter_ns
                .saturating_add(left.media.jitter_ns)
                .saturating_add(right.media.jitter_ns),
            bandwidth_bps: self
                .base
                .bandwidth_bps
                .min(left.media.bandwidth_bps)
                .min(right.media.bandwidth_bps),
            loss_ppm: combined_loss(self.base.loss_ppm, left.loss_ppm, right.loss_ppm),
            duplication_ppm: self.base.duplication_ppm,
            ordering: if left.media.ordering == MediaOrdering::Stream
                || right.media.ordering == MediaOrdering::Stream
            {
                LinkOrdering::Stream
            } else {
                self.base.ordering
            },
            mtu_bytes: self
                .base
                .mtu_bytes
                .min(left.media.effective_mtu_bytes)
                .min(right.media.effective_mtu_bytes),
            queue_bytes: self
                .base
                .queue_bytes
                .min(left.queue_bytes)
                .min(right.queue_bytes),
            transport_overhead_bytes: left
                .media
                .transport_overhead_bytes
                .max(right.media.transport_overhead_bytes),
        }
    }
}

fn parse_profiles(
    campaign: &Value,
    base: &LinkConfig,
) -> Result<Vec<NodeTransportProfile>, RunError> {
    let values = campaign
        .pointer("/transports/profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RunError::Unsupported("random-mixed requires transport profiles".to_owned())
        })?;
    if values.is_empty() {
        return Err(RunError::Unsupported(
            "random-mixed requires at least one transport profile".to_owned(),
        ));
    }
    values
        .iter()
        .map(|value| {
            let name = value.get("name").and_then(Value::as_str).ok_or_else(|| {
                RunError::Unsupported("transport profile requires a name".to_owned())
            })?;
            let kind_name = value.get("type").and_then(Value::as_str).ok_or_else(|| {
                RunError::Unsupported("transport profile requires a type".to_owned())
            })?;
            let kind = MediaKind::parse(kind_name).ok_or_else(|| {
                RunError::Unsupported(format!("unsupported transport profile type {kind_name}"))
            })?;
            let mut media = builtin_profile(kind);
            media.id = name.to_owned();
            media.effective_mtu_bytes = value
                .get("mtu_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(media.effective_mtu_bytes);
            media.latency_ns = duration_value(value.get("latency"))?.unwrap_or(media.latency_ns);
            media.jitter_ns = duration_value(value.get("jitter"))?.unwrap_or(media.jitter_ns);
            media.bandwidth_bps = value
                .get("bandwidth_bps")
                .and_then(Value::as_u64)
                .unwrap_or(media.bandwidth_bps);
            Ok(NodeTransportProfile {
                name: name.to_owned(),
                media,
                loss_ppm: value.get("loss_ppm").and_then(Value::as_u64).unwrap_or(0) as u32,
                queue_bytes: value
                    .get("queue_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(base.queue_bytes),
                weight: value.get("weight").and_then(Value::as_u64).unwrap_or(1),
            })
        })
        .collect()
}

fn homogeneous(name: &str, kind: MediaKind, base: &LinkConfig) -> NodeTransportProfile {
    let mut media = builtin_profile(kind);
    media.id = name.to_owned();
    media.effective_mtu_bytes = base.mtu_bytes;
    media.latency_ns = base.latency_ns;
    media.jitter_ns = base.jitter_ns;
    media.bandwidth_bps = base.bandwidth_bps;
    NodeTransportProfile {
        name: name.to_owned(),
        media,
        loss_ppm: base.loss_ppm,
        queue_bytes: base.queue_bytes,
        weight: 1,
    }
}

fn combined_loss(base: u32, left: u32, right: u32) -> u32 {
    let survive = [base, left, right]
        .into_iter()
        .fold(1_000_000_u128, |acc, loss| {
            acc * u128::from(1_000_000_u32.saturating_sub(loss)) / 1_000_000
        });
    (1_000_000_u128.saturating_sub(survive)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_losses_combine_without_exceeding_one_million() {
        assert_eq!(combined_loss(0, 0, 0), 0);
        assert_eq!(combined_loss(1_000_000, 0, 0), 1_000_000);
        assert_eq!(combined_loss(100_000, 100_000, 0), 190_000);
    }

    #[test]
    fn authored_profile_jitter_contributes_to_effective_edge() {
        let campaign = json!({
            "transports": {
                "assignment": "random-mixed",
                "profiles": [{
                    "name": "wifi-test", "type": "wifi", "mtu_bytes": 1500,
                    "jitter": {"nanoseconds": 7}, "weight": 1
                }]
            }
        });
        let base = LinkConfig {
            jitter_ns: 3,
            ..LinkConfig::default()
        };
        let plan = TransportPlan::from_campaign(&campaign, 2, 9, base).unwrap();
        assert_eq!(plan.link_config(0, 1).jitter_ns, 17);
    }
}
