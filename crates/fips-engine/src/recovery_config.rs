use super::*;

impl RecoveryConfig {
    pub(super) fn from_plan(
        plan: &NormalizedPlan,
        root: &RootRatchetReport,
    ) -> Result<Self, RecoveryError> {
        let campaign = &plan.campaign;
        let nodes = u32::try_from(root.node_count)
            .map_err(|_| RecoveryError::Unsupported("M2 requires u32 node IDs".to_owned()))?;
        let bloom_mode = match string_at(campaign, "/fidelity/bloom").unwrap_or("exact-bits") {
            "exact-bits" => BloomMode::ExactBits,
            "sparse-bits" => BloomMode::SparseBits,
            "occupancy" => BloomMode::Occupancy,
            other => return Err(RecoveryError::Unsupported(format!("Bloom mode {other}"))),
        };
        let traffic_model =
            TrafficModel::parse(string_at(campaign, "/traffic/model").unwrap_or("idle"))?;
        let payload_bytes = u64_at(campaign, "/traffic/payload_bytes").unwrap_or(512);
        let rate_bps = u64_at(campaign, "/traffic/rate_bps").unwrap_or(4_096_000);
        let interval_ns = payload_bytes
            .saturating_mul(8)
            .saturating_mul(1_000_000_000)
            .checked_div(rate_bps.max(1))
            .unwrap_or(1_000_000)
            .max(1);
        let cpu_units = u64_at(campaign, "/resources/node_profiles/0/cpu_units").unwrap_or(1_000);
        let memory = u64_at(campaign, "/resources/node_profiles/0/memory_bytes").unwrap_or(1 << 30);
        let queue = u64_at(campaign, "/resources/node_profiles/0/queue_bytes").unwrap_or(1 << 20);
        let tables =
            u64_at(campaign, "/resources/node_profiles/0/table_entries").unwrap_or(100_000);
        let mut resource_profile = ResourceProfile::baseline();
        resource_profile.cpu_units_per_ms = cpu_units;
        resource_profile
            .capacities
            .insert(ResourceKind::AllocationBytes, memory);
        resource_profile
            .capacities
            .insert(ResourceKind::QueueBytes, queue);
        resource_profile
            .capacities
            .insert(ResourceKind::CacheEntries, tables);
        let ordering = match string_at(campaign, "/links/ordering").unwrap_or("stream") {
            "datagram" => LinkOrdering::Datagram,
            "stream" => LinkOrdering::Stream,
            other => return Err(RecoveryError::Unsupported(format!("link ordering {other}"))),
        };
        let latency_ns = duration_at(campaign, "/links/latency").unwrap_or(1_000_000);
        let link = LinkConfig {
            latency_ns,
            jitter_ns: duration_at(campaign, "/links/jitter").unwrap_or(latency_ns / 2),
            bandwidth_bps: u64_at(campaign, "/links/bandwidth_bps").unwrap_or(10_000_000),
            loss_ppm: u32::try_from(u64_at(campaign, "/links/loss_ppm").unwrap_or(0))
                .map_err(|_| RecoveryError::Unsupported("loss_ppm exceeds u32".to_owned()))?,
            duplication_ppm: u32::try_from(u64_at(campaign, "/links/duplication_ppm").unwrap_or(0))
                .map_err(|_| {
                    RecoveryError::Unsupported("duplication_ppm exceeds u32".to_owned())
                })?,
            ordering,
            mtu_bytes: u64_at(campaign, "/links/mtu_bytes").unwrap_or(1_500),
            queue_bytes: u64_at(campaign, "/links/queue_bytes").unwrap_or(1 << 20),
            transport_overhead_bytes: 28,
        };
        let lookup_mtu = link.mtu_bytes.saturating_sub(link.transport_overhead_bytes);
        let arrival_start_ns =
            duration_at(campaign, "/identities/arrivals/schedule/start").unwrap_or(2_000_000_000);
        let arrival_interval_ns =
            duration_at(campaign, "/identities/arrivals/schedule/interval").unwrap_or(499_000_000);
        let last_arrival_ns = arrival_start_ns.saturating_add(
            u64::from(
                u32::try_from(root.arrivals)
                    .unwrap_or(u32::MAX)
                    .saturating_sub(1),
            )
            .saturating_mul(arrival_interval_ns),
        );
        Ok(Self {
            nodes,
            arrivals: u32::try_from(root.arrivals)
                .map_err(|_| RecoveryError::Unsupported("arrival count exceeds u32".to_owned()))?,
            arrival_start_ns,
            arrival_interval_ns,
            tree_recovery_ns: root.quiescence_ns.saturating_sub(last_arrival_ns),
            bloom_mode,
            bloom_debounce_ns: duration_at(campaign, "/protocol/parameters/bloom_update_debounce")
                .unwrap_or(DEFAULT_BLOOM_DEBOUNCE_NS),
            bloom_max_fpr: decimal_at(campaign, "/protocol/parameters/bloom_max_fpr")
                .or_else(|| {
                    u64_at(campaign, "/protocol/parameters/bloom_max_fpr_ppm")
                        .map(|value| value as f64 / 1_000_000.0)
                })
                .unwrap_or(0.20),
            cache_entries: usize::try_from(
                u64_at(campaign, "/protocol/parameters/coord_cache_entries").unwrap_or(64),
            )
            .map_err(|_| RecoveryError::Unsupported("cache capacity exceeds usize".to_owned()))?,
            cache_ttl_ns: duration_at(campaign, "/protocol/parameters/coord_cache_ttl")
                .unwrap_or(5_000_000_000),
            lookup: LookupConfig {
                ttl: u8::try_from(
                    u64_at(campaign, "/protocol/parameters/lookup_ttl").unwrap_or(64),
                )
                .map_err(|_| RecoveryError::Unsupported("lookup TTL exceeds u8".to_owned()))?,
                maximum_attempts: u32::try_from(
                    u64_at(campaign, "/protocol/parameters/lookup_attempts").unwrap_or(3),
                )
                .map_err(|_| RecoveryError::Unsupported("lookup attempts exceed u32".to_owned()))?,
                mtu_bytes: lookup_mtu,
                ..LookupConfig::default()
            },
            traffic: TrafficConfig {
                model: traffic_model,
                nodes,
                flow_count: u64_at(campaign, "/traffic/parameters/flow_count").unwrap_or(128),
                payload_bytes,
                rate_bps,
                interval_ns,
                segments_per_stream: u32::try_from(
                    u64_at(campaign, "/traffic/parameters/segments_per_stream").unwrap_or(32),
                )
                .map_err(|_| {
                    RecoveryError::Unsupported("segments_per_stream exceeds u32".to_owned())
                })?,
                burst_size: u32::try_from(
                    u64_at(campaign, "/traffic/parameters/burst_size").unwrap_or(16),
                )
                .map_err(|_| RecoveryError::Unsupported("burst_size exceeds u32".to_owned()))?,
                burst_interval_ns: u64_at(campaign, "/traffic/parameters/burst_interval_ns")
                    .unwrap_or(250_000_000),
                seed: plan.seed,
                transfers: Vec::new(),
            },
            link,
            resource_profile,
            heterogeneous: string_at(campaign, "/resources/assignment")
                .is_some_and(|value| value == "heterogeneous"),
        })
    }
}
