use super::*;

impl<'a> CoupledState<'a> {
    pub(super) fn new(
        seed: u64,
        root: &'a RootRatchetReport,
        config: RecoveryConfig,
    ) -> Result<Self, RecoveryError> {
        let mut bloom_model = BloomModel::new(config.bloom_mode);
        for node in 0..config.nodes {
            bloom_model.insert(&node.to_le_bytes());
        }
        let traffic_plan = TrafficPlan::generate(&config.traffic)?;
        let mut cache = CoordinateCache::new(config.cache_entries, config.cache_ttl_ns);
        let initial_root = root
            .root_generations
            .first()
            .map(|value| hex_16(value))
            .transpose()?
            .unwrap_or([0xff; 16]);
        for destination in traffic_plan
            .flows
            .iter()
            .map(|flow| flow.destination)
            .collect::<BTreeSet<_>>()
        {
            cache.insert(
                node_key(destination),
                initial_root,
                chain_path(destination),
                0,
            );
        }
        let mut resources = Vec::with_capacity(config.nodes as usize);
        for node in 0..config.nodes {
            let mut profile = config.resource_profile.clone();
            if config.heterogeneous && (node == 0 || node + 1 == config.nodes) {
                profile.name = if node == 0 { "slow-root" } else { "slow-leaf" }.to_owned();
                profile.cpu_units_per_ms = profile.cpu_units_per_ms.max(10) / 10;
            }
            resources.push(ResourcePool::new(profile));
        }
        let link = LinkService::uniform(seed, 1, config.link.clone());
        Ok(Self {
            seed,
            root,
            bloom_wave: BloomReplacementWave::new(config.bloom_debounce_ns),
            cache,
            lookup: LookupService::new(config.lookup.clone()),
            traffic_plan,
            resources,
            link,
            link_delivery_ns: 0,
            traffic: TrafficRecovery::default(),
            lookup_counters: LookupCounters::default(),
            resource_exhaustions: Vec::new(),
            ledger: Vec::new(),
            projections: CostProjections::default(),
            accepted_frame_bytes: 0,
            logical_wire_bytes: 0,
            maximum_frame_bytes: 0,
            arrival_summaries: Vec::new(),
            current_root: initial_root,
            latest_arrival_cause: None,
            last_lookup_ns: 0,
            useful_delivery_times: Vec::new(),
            bloom_model,
            config,
            last_useful_delivery_ns: None,
        })
    }
}
