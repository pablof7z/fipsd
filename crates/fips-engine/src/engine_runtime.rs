use super::*;

impl Simulation {
    pub(super) fn new(plan: NormalizedPlan, config: RunConfig) -> Result<Self, RunError> {
        let mut graph = GraphStore::generate(
            config.topology,
            config.nodes,
            config.average_degree,
            plan.seed,
            &config.explicit_edges,
        )?;
        graph.reserve_arrivals(config.reserved_arrivals)?;
        for edge in 0..graph.edge_count() as EdgeId {
            let (left, right) = graph.edge(edge)?;
            if !graph.is_active(left) || !graph.is_active(right) {
                graph.set_edge_active(edge, false)?;
            }
        }
        let initial_root = graph
            .node_ids()
            .filter(|id| graph.is_active(*id))
            .min_by_key(|id| graph.address(*id).ok())
            .ok_or_else(|| RunError::Invariant("no initial root".to_owned()))?;
        let root_address = graph.address(initial_root)?;
        let mut root_generations = BTreeSet::new();
        root_generations.insert(root_address);
        let edge_count = graph.edge_count();
        let node_count = graph.node_count();
        let seed = plan.seed;
        let recovery = GraphRecoveryRuntime::from_plan(&plan, config.nodes)?;
        let traffic = RoutedTrafficRuntime::from_plan(&plan, config.nodes)?;
        let bloom = StreamedBloomRuntime::from_plan(&plan, &graph)?;
        let transports =
            TransportPlan::from_campaign(&plan.campaign, config.nodes, seed, config.link.clone())?;
        let media_zones = MediaZonePlan::from_plan(&plan, config.nodes)?;
        let mut links = LinkService::uniform(seed, edge_count, config.link.clone());
        for edge in 0..edge_count as u32 {
            let (from, to) = graph.edge(edge)?;
            media_zones.configure_edge(edge, from, to, &transports, &mut links)?;
        }
        Ok(Self {
            plan,
            config: config.clone(),
            graph,
            scheduler: Scheduler::new(MAX_EVENTS),
            links,
            partition_blocks: vec![0; edge_count],
            failed_transport_classes: BTreeSet::new(),
            parent_cost_ppm: vec![1_000_000; edge_count],
            transports,
            media_zones,
            peer_views: BTreeMap::new(),
            pending: BTreeMap::new(),
            last_sent_ns: BTreeMap::new(),
            sent_times: BTreeMap::new(),
            last_parent_switch_ns: vec![None; node_count],
            trace: Vec::new(),
            ledger: BTreeMap::new(),
            tree: TreeAnnounceCounters::default(),
            root_generations,
            parent_transitions: 0,
            identity_trials: 0,
            accepted_arrivals: 0,
            authenticated_sybil_arrivals: 0,
            traffic,
            bloom,
            recovery,
        })
    }

    pub(super) fn run_with_observer(
        mut self,
        observer: &mut impl FnMut(&EventRecord) -> Result<(), String>,
    ) -> Result<Self, RunError> {
        self.schedule_inputs()?;
        while let Some(event) = self.scheduler.pop() {
            if self.trace.len() >= MAX_EVENTS {
                return Err(RunError::Invariant(format!(
                    "event limit {MAX_EVENTS} exceeded"
                )));
            }
            let event_id = format!("event-{:016x}", event.id);
            let parent = event.parent.map(|id| format!("event-{id:016x}"));
            let event_kind = event.payload.kind().to_owned();
            let data = match event.payload {
                SimEvent::InitialAnnounce => self.handle_initial(event.id, &event_id)?,
                SimEvent::Activate {
                    node,
                    ordinal,
                    lower_root,
                    targets,
                } => {
                    self.handle_activate(event.id, &event_id, node, ordinal, lower_root, &targets)?
                }
                SimEvent::AnnounceDue { from, to, cause } => {
                    self.handle_announce_due(event.id, &event_id, from, to, &cause)?
                }
                SimEvent::DeliverAnnounce {
                    delivery,
                    snapshot,
                    cause,
                } => self.handle_delivery(event.id, &event_id, &delivery, snapshot, &cause)?,
                SimEvent::InjectParentLoop => {
                    return Err(RunError::Invariant(
                        "loop-freedom: injected parent ancestry contains the receiving node"
                            .to_owned(),
                    ));
                }
                SimEvent::Deactivate { node } => {
                    self.handle_deactivate(event.id, &event_id, node)?
                }
                SimEvent::Reappear { node } => self.handle_reappear(event.id, &event_id, node)?,
                SimEvent::NetworkCut { input } => {
                    self.handle_network_cut(event.id, &event_id, input)?
                }
                SimEvent::LinkUpdate { input } => {
                    self.handle_link_update(event.id, &event_id, input)?
                }
                SimEvent::SessionRekey { input } => {
                    self.handle_session_rekey(event.id, &event_id, input)?
                }
                SimEvent::SessionRekeyCompleted { completion } => {
                    self.handle_session_rekey_completed(&event_id, completion)?
                }
                SimEvent::ExpireCoordinateCache { input } => {
                    self.handle_cache_expiry(&event_id, input)?
                }
                SimEvent::LookupWave { input } => {
                    self.handle_lookup_wave(event.id, &event_id, input)?
                }
                SimEvent::TransportClass { input } => {
                    self.handle_transport_class(event.id, &event_id, input)?
                }
                SimEvent::ParentCost { input } => {
                    self.handle_parent_cost(event.id, &event_id, input)?
                }
                SimEvent::SybilArrival { input, node } => {
                    self.handle_sybil_arrival(event.id, &event_id, input, node)?
                }
                SimEvent::TrafficOffer { index } => {
                    self.handle_traffic_offer(event.id, &event_id, index)?
                }
                SimEvent::TrafficHopDue { frame } => {
                    self.handle_traffic_hop(event.id, &event_id, frame)?
                }
                SimEvent::DeliverTraffic {
                    delivery,
                    frame,
                    forward_copy,
                } => self.handle_traffic_delivery(
                    event.id,
                    &event_id,
                    delivery,
                    frame,
                    forward_copy,
                )?,
                SimEvent::BloomDue { from, to, cause } => {
                    self.handle_bloom_due(event.id, &event_id, from, to, &cause)?
                }
                SimEvent::DeliverBloom {
                    delivery,
                    snapshot,
                    cause,
                } => {
                    self.handle_bloom_delivery(event.id, &event_id, &delivery, snapshot, &cause)?
                }
                SimEvent::LookupStart { index, attempt } => {
                    self.handle_lookup_start(event.id, &event_id, index, attempt)?
                }
                SimEvent::RecoveryHopDue { frame } => {
                    self.handle_recovery_hop(event.id, &event_id, frame)?
                }
                SimEvent::DeliverRecovery {
                    delivery,
                    frame,
                    forward_copy,
                } => self.handle_recovery_delivery(
                    event.id,
                    &event_id,
                    delivery,
                    frame,
                    forward_copy,
                )?,
            };
            if event_kind.starts_with("data.") {
                self.traffic.as_mut().unwrap().counters.quiescence_ns = event.virtual_time_ns;
            }
            if event_kind.starts_with("bloom.") {
                self.bloom.as_mut().unwrap().counters.quiescence_ns = event.virtual_time_ns;
            }
            if event_kind.starts_with("lookup.") || event_kind.starts_with("session.") {
                self.recovery.as_mut().unwrap().counters.quiescence_ns = event.virtual_time_ns;
            }
            let record = EventRecord {
                event_id,
                virtual_time_ns: event.virtual_time_ns,
                ordinal: event.ordinal,
                kind: event_kind,
                causal_parent: parent,
                data,
            };
            observer(&record).map_err(RunError::Observer)?;
            self.trace.push(record);
        }
        Ok(self)
    }
}
