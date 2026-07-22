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
        graph.reserve_arrivals(config.arrivals)?;
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
        Ok(Self {
            plan,
            config: config.clone(),
            graph,
            scheduler: Scheduler::new(MAX_EVENTS),
            links: LinkService::uniform(seed, edge_count, config.link),
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
        })
    }

    pub(super) fn run(mut self) -> Result<Self, RunError> {
        self.scheduler
            .schedule_at(0, None, SimEvent::InitialAnnounce)?;
        let first_arrival = self.graph.node_count() as u32 - self.config.arrivals;
        for ordinal in 0..self.config.arrivals {
            let at = self
                .config
                .arrival_interval_ns
                .checked_mul(u64::from(ordinal))
                .and_then(|offset| self.config.arrival_start_ns.checked_add(offset))
                .ok_or(RunError::Arithmetic)?;
            self.scheduler.schedule_at(
                at,
                None,
                SimEvent::Activate {
                    node: first_arrival + ordinal,
                    ordinal,
                },
            )?;
        }
        if let Some(at) = self.config.inject_parent_loop_at_ns {
            self.scheduler
                .schedule_at(at, None, SimEvent::InjectParentLoop)?;
        }
        for lifecycle in &self.config.lifecycle {
            let payload = if lifecycle.reappear {
                SimEvent::Reappear {
                    node: lifecycle.node,
                }
            } else {
                SimEvent::Deactivate {
                    node: lifecycle.node,
                }
            };
            self.scheduler.schedule_at(lifecycle.at_ns, None, payload)?;
        }

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
                SimEvent::Activate { node, ordinal } => {
                    self.handle_activate(event.id, &event_id, node, ordinal)?
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
            };
            self.trace.push(EventRecord {
                event_id,
                virtual_time_ns: event.virtual_time_ns,
                ordinal: event.ordinal,
                kind: event_kind,
                causal_parent: parent,
                data,
            });
        }
        Ok(self)
    }
}
