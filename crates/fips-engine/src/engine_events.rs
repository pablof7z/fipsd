use super::*;

impl Simulation {
    pub(super) fn handle_initial(
        &mut self,
        event: EventId,
        evidence: &str,
    ) -> Result<Value, RunError> {
        let cause = "input:initial-topology";
        self.add_ledger(cause, "performed", 1, evidence);
        let active = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .collect::<Vec<_>>();
        for node in &active {
            self.request_all(*node, cause, Some(event))?;
        }
        Ok(json!({"active_nodes": active.len()}))
    }

    pub(super) fn handle_activate(
        &mut self,
        event: EventId,
        evidence: &str,
        node: NodeId,
        ordinal: u32,
    ) -> Result<Value, RunError> {
        let cause = format!("input:arrival-{ordinal:04}");
        self.add_ledger(&cause, "requested", 1, evidence);
        let minimum = self
            .graph
            .node_ids()
            .filter(|id| self.graph.is_active(*id))
            .map(|id| self.graph.address(id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min()
            .ok_or_else(|| RunError::Invariant("arrival has no visible root".to_owned()))?;
        let (address, trials) = if self.config.address_policy == "precomputed-ladder" {
            (self.config.precomputed_ladder[ordinal as usize], 0_u64)
        } else {
            (
                minimum.one_lower().ok_or_else(|| {
                    RunError::Invariant("cannot generate an address below the zero root".to_owned())
                })?,
                1_u64,
            )
        };
        if address >= minimum {
            return Err(RunError::Invariant(format!(
                "arrival {ordinal} address {} is not lower than visible root {}",
                address.to_hex(),
                minimum.to_hex()
            )));
        }
        if self.config.attacker_budget_mode == "bounded"
            && self
                .config
                .attacker_operations
                .is_none_or(|budget| self.identity_trials + trials > budget)
        {
            return Err(RunError::BudgetExhausted {
                required: self.identity_trials + trials,
                available: self.config.attacker_operations.unwrap_or(0),
            });
        }
        self.identity_trials += trials;
        self.graph.set_address(node, address)?;
        self.graph.reset_self_root(node)?;
        let target = self.graph.select_attachment(
            self.config.attachment,
            self.plan.seed,
            u64::from(ordinal),
        )?;
        self.graph.set_active(node, true)?;
        let edge = if let Some(edge) = self.graph.edge_between(node, target) {
            edge
        } else {
            let edge = self.graph.add_edge(node, target)?;
            self.links.set_config(edge, self.config.link.clone())?;
            edge
        };
        self.accepted_arrivals += 1;
        self.root_generations.insert(address);
        self.add_ledger(&cause, "performed", 1, evidence);
        self.add_ledger(&cause, "identity-generation-operations", trials, evidence);
        self.request_all(node, &cause, Some(event))?;
        self.request_announce(target, node, &cause, Some(event))?;
        Ok(json!({
            "node": node,
            "address": address.to_hex(),
            "address_policy": self.config.address_policy,
            "attachment": format!("{:?}", self.config.attachment),
            "target": target,
            "edge": edge,
            "identity_trials": trials
        }))
    }

    pub(super) fn handle_announce_due(
        &mut self,
        event: EventId,
        evidence: &str,
        from: NodeId,
        to: NodeId,
        cause: &str,
    ) -> Result<Value, RunError> {
        if self.pending.get(&(from, to)).map(|item| item.event_id) != Some(event) {
            return Err(RunError::Invariant(format!(
                "orphaned announce event {event} for {from}->{to}"
            )));
        }
        let pending_cause = self.pending.remove(&(from, to)).map(|item| item.cause);
        if pending_cause.as_deref() != Some(cause) {
            return Err(RunError::Invariant(format!(
                "announce cause drift for {from}->{to}"
            )));
        }
        if !self.graph.is_active(from) || !self.graph.is_active(to) {
            self.tree.cancelled += 1;
            self.add_ledger(cause, "cancelled", 1, evidence);
            return Ok(json!({"from": from, "to": to, "skipped": "inactive"}));
        }
        let snapshot = self.snapshot(from)?;
        let depth = snapshot.ancestry.len().saturating_sub(1) as u32;
        let manifest = CodecManifest::load().map_err(|error| RunError::Codec(error.to_string()))?;
        if depth > manifest.maximum_safe_tree_depth {
            return Err(RunError::Unsupported(format!(
                "TreeAnnounce depth {depth} exceeds FMP maximum {}",
                manifest.maximum_safe_tree_depth
            )));
        }
        let frame_bytes = 168_u64 + 32 * u64::from(depth);
        self.tree.constructed += 1;
        self.tree.signed += 1;
        self.tree.serialized += 1;
        self.add_ledger(cause, "constructed", 1, evidence);
        self.add_ledger(cause, "signed", 1, evidence);
        self.add_ledger(cause, "serialized", frame_bytes, evidence);
        let edge = self
            .graph
            .edge_between(from, to)
            .ok_or_else(|| RunError::Invariant(format!("no edge for announce {from}->{to}")))?;
        match self.links.enqueue(EnqueueRequest {
            edge_id: edge,
            from,
            to,
            class: LinkClass::Control,
            frame_bytes,
            useful_payload_bytes: 0,
            now_ns: self.scheduler.now_ns(),
        }) {
            Ok(result) => {
                self.tree.queued += 1;
                self.tree.transmitted += result.transmitted_bytes
                    / (frame_bytes + self.config.link.transport_overhead_bytes);
                self.tree.transmitted_frame_bytes += result.transmitted_bytes.saturating_sub(
                    self.config.link.transport_overhead_bytes
                        * (result.transmitted_bytes
                            / (frame_bytes + self.config.link.transport_overhead_bytes)),
                );
                self.add_ledger(cause, "queued", frame_bytes, evidence);
                self.add_ledger(cause, "transmitted", result.transmitted_bytes, evidence);
                for delivery in result.deliveries {
                    self.scheduler.schedule_at(
                        delivery.deliver_at_ns,
                        Some(event),
                        SimEvent::DeliverAnnounce {
                            delivery,
                            snapshot: snapshot.clone(),
                            cause: cause.to_owned(),
                        },
                    )?;
                }
                self.last_sent_ns
                    .insert((from, to), self.scheduler.now_ns());
                self.sent_times
                    .entry((from, to))
                    .or_default()
                    .push(self.scheduler.now_ns());
                Ok(json!({
                    "from": from,
                    "to": to,
                    "depth": depth,
                    "frame_bytes": frame_bytes,
                    "transport_bytes": result.transmitted_bytes,
                    "lost_bytes": result.lost_bytes,
                    "queue_occupancy_bytes": result.queue_occupancy_bytes
                }))
            }
            Err(error @ (LinkError::MtuExceeded { .. } | LinkError::QueueFull { .. })) => {
                self.tree.rejected += 1;
                self.add_ledger(cause, "rejected", frame_bytes, evidence);
                Ok(json!({
                    "from": from,
                    "to": to,
                    "depth": depth,
                    "frame_bytes": frame_bytes,
                    "rejected": error.to_string()
                }))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn handle_deactivate(
        &mut self,
        event: EventId,
        evidence: &str,
        node: NodeId,
    ) -> Result<Value, RunError> {
        if !self.graph.is_active(node) {
            return Err(RunError::Invariant(format!(
                "cannot disappear inactive node {node}"
            )));
        }
        let cause = format!("input:disappear-{node}");
        let neighbors = self.graph.active_neighbors(node);
        self.graph.set_active(node, false)?;
        self.peer_views
            .retain(|(receiver, sender), _| *receiver != node && *sender != node);
        for neighbor in neighbors {
            if self.graph.parent(neighbor) == Some(node) {
                if let Some(transition) = self.evaluate_parent(neighbor)? {
                    self.apply_transition(neighbor, transition, &cause, Some(event), evidence)?;
                } else {
                    self.graph.reset_self_root(neighbor)?;
                    self.root_generations.insert(self.graph.address(neighbor)?);
                    self.request_all(neighbor, &cause, Some(event))?;
                }
            }
        }
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({"node": node, "active": false}))
    }

    pub(super) fn handle_reappear(
        &mut self,
        event: EventId,
        evidence: &str,
        node: NodeId,
    ) -> Result<Value, RunError> {
        if self.graph.is_active(node) {
            return Err(RunError::Invariant(format!(
                "cannot reappear active node {node}"
            )));
        }
        let cause = format!("input:reappear-{node}");
        self.graph.set_active(node, true)?;
        self.graph.reset_self_root(node)?;
        self.request_all(node, &cause, Some(event))?;
        for neighbor in self.graph.active_neighbors(node) {
            self.request_announce(neighbor, node, &cause, Some(event))?;
        }
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({"node": node, "active": true}))
    }

    pub(super) fn handle_delivery(
        &mut self,
        event: EventId,
        evidence: &str,
        delivery: &Delivery,
        snapshot: TreeSnapshot,
        cause: &str,
    ) -> Result<Value, RunError> {
        self.links.record_delivery(delivery, 0)?;
        self.tree.delivered += 1;
        self.tree.delivered_frame_bytes += delivery.frame_bytes;
        self.add_ledger(cause, "delivered", delivery.wire_bytes, evidence);
        let receiver = delivery.to;
        if !self.graph.is_active(receiver) {
            return Ok(json!({
                "edge": delivery.edge_id,
                "from": delivery.from,
                "to": receiver,
                "frame_bytes": delivery.frame_bytes,
                "skipped": "receiver-inactive"
            }));
        }
        if !self.snapshot_semantics_valid(&snapshot)? {
            return Err(RunError::Invariant(format!(
                "accepted-ancestry: invalid snapshot from {}",
                delivery.from
            )));
        }
        let key = (receiver, delivery.from);
        let fresh = self
            .peer_views
            .get(&key)
            .is_none_or(|old| snapshot.sequence > old.sequence || snapshot != *old);
        if fresh {
            self.peer_views.insert(key, snapshot);
        }
        let transition = fresh
            .then(|| self.evaluate_parent(receiver))
            .transpose()?
            .flatten();
        if let Some(transition) = transition {
            self.apply_transition(receiver, transition, cause, Some(event), evidence)?;
        }
        Ok(json!({
            "edge": delivery.edge_id,
            "from": delivery.from,
            "to": delivery.to,
            "frame_bytes": delivery.frame_bytes,
            "copy": delivery.copy_ordinal,
            "fresh": fresh,
            "root": self.graph.address(self.graph.root(receiver))?.to_hex(),
            "parent": self.graph.parent(receiver)
        }))
    }

    pub(super) fn request_all(
        &mut self,
        from: NodeId,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        let neighbors = self.graph.active_neighbors(from);
        for to in neighbors {
            self.request_announce(from, to, cause, parent)?;
        }
        Ok(())
    }

    pub(super) fn request_announce(
        &mut self,
        from: NodeId,
        to: NodeId,
        cause: &str,
        parent: Option<EventId>,
    ) -> Result<(), RunError> {
        self.tree.requested += 1;
        self.add_ledger(cause, "requested", 1, "");
        let due = self
            .last_sent_ns
            .get(&(from, to))
            .map_or(self.scheduler.now_ns(), |last| {
                self.scheduler
                    .now_ns()
                    .max(last.saturating_add(self.config.debounce_ns))
            });
        if let Some(previous_cause) = self
            .pending
            .get(&(from, to))
            .map(|previous| previous.cause.clone())
        {
            self.tree.superseded += 1;
            self.tree.coalesced += 1;
            self.add_ledger(&previous_cause, "superseded", 1, "");
            self.add_ledger(cause, "coalesced", 1, "");
        }
        let key = format!("announce:{from}:{to}");
        let event_id = self.scheduler.schedule_coalesced(
            key,
            due,
            parent,
            SimEvent::AnnounceDue {
                from,
                to,
                cause: cause.to_owned(),
            },
        )?;
        self.pending.insert(
            (from, to),
            PendingAnnounce {
                event_id,
                cause: cause.to_owned(),
            },
        );
        Ok(())
    }
}
