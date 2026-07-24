use super::*;

impl Simulation {
    pub(super) fn handle_network_cut(
        &mut self,
        event: EventId,
        evidence: &str,
        input: NetworkCutInput,
    ) -> Result<Value, RunError> {
        let cause = format!("input:{}", input.id);
        let group = input.nodes.iter().copied().collect::<BTreeSet<_>>();
        let crossing = (0..self.graph.edge_count() as EdgeId)
            .filter_map(|edge| {
                let (left, right) = self.graph.edge(edge).ok()?;
                (group.contains(&left) != group.contains(&right)).then_some((edge, left, right))
            })
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for (edge, left, right) in crossing {
            if input.enabled {
                self.partition_blocks[edge as usize] =
                    self.partition_blocks[edge as usize].saturating_sub(1);
                if self.partition_blocks[edge as usize] == 0 && !self.graph.is_edge_active(edge) {
                    self.graph.set_edge_active(edge, true)?;
                    changed.push((edge, left, right));
                }
            } else {
                self.partition_blocks[edge as usize] =
                    self.partition_blocks[edge as usize].saturating_add(1);
                if self.graph.is_edge_active(edge) {
                    self.graph.set_edge_active(edge, false)?;
                    changed.push((edge, left, right));
                }
            }
        }
        if input.enabled {
            self.reconnect_edges(event, &cause, &changed)?;
        } else {
            self.disconnect_edges(event, evidence, &cause, &changed)?;
        }
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({
            "intervention_id": input.id,
            "nodes": input.nodes,
            "edge_state": if input.enabled { "active" } else { "partitioned" },
            "changed_edges": changed.iter().map(|(edge, left, right)| {
                json!({"id": edge, "from": left, "to": right})
            }).collect::<Vec<_>>()
        }))
    }

    pub(super) fn handle_transport_class(
        &mut self,
        event: EventId,
        evidence: &str,
        input: TransportClassInput,
    ) -> Result<Value, RunError> {
        if !self.transports.contains_profile(&input.profile) {
            return Err(RunError::Unsupported(format!(
                "unknown transport profile {}",
                input.profile
            )));
        }
        let changed_membership = if input.restore {
            self.failed_transport_classes.remove(&input.profile)
        } else {
            self.failed_transport_classes.insert(input.profile.clone())
        };
        if !changed_membership {
            return Err(RunError::Unsupported(format!(
                "transport profile {} is already {}",
                input.profile,
                if input.restore { "active" } else { "failed" }
            )));
        }
        let nodes = self
            .graph
            .node_ids()
            .filter(|node| self.transports.profile(*node).name == input.profile)
            .collect::<Vec<_>>();
        let node_set = nodes.iter().copied().collect::<BTreeSet<_>>();
        let affected = (0..self.graph.edge_count() as EdgeId)
            .filter_map(|edge| {
                let (left, right) = self.graph.edge(edge).ok()?;
                (node_set.contains(&left) || node_set.contains(&right))
                    .then_some((edge, left, right))
            })
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for (edge, left, right) in affected {
            if input.restore {
                self.partition_blocks[edge as usize] =
                    self.partition_blocks[edge as usize].saturating_sub(1);
                if self.partition_blocks[edge as usize] == 0 && !self.graph.is_edge_active(edge) {
                    self.graph.set_edge_active(edge, true)?;
                    changed.push((edge, left, right));
                }
            } else {
                self.partition_blocks[edge as usize] =
                    self.partition_blocks[edge as usize].saturating_add(1);
                if self.graph.is_edge_active(edge) {
                    self.graph.set_edge_active(edge, false)?;
                    changed.push((edge, left, right));
                }
            }
        }
        let cause = format!("input:{}", input.id);
        if input.restore {
            self.reconnect_edges(event, &cause, &changed)?;
        } else {
            self.disconnect_edges(event, evidence, &cause, &changed)?;
        }
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({
            "intervention_id": input.id,
            "profile": input.profile,
            "profile_state": if input.restore { "active" } else { "failed" },
            "affected_nodes": nodes,
            "changed_edges": changed.iter().map(|(edge, left, right)| {
                json!({"id": edge, "from": left, "to": right})
            }).collect::<Vec<_>>()
        }))
    }

    fn disconnect_edges(
        &mut self,
        event: EventId,
        evidence: &str,
        cause: &str,
        edges: &[(EdgeId, NodeId, NodeId)],
    ) -> Result<(), RunError> {
        let mut invalidated = 0;
        let mut disrupted = 0;
        for (_, left, right) in edges {
            self.peer_views.remove(&(*left, *right));
            self.peer_views.remove(&(*right, *left));
            self.disconnect_bloom_edge(*left, *right);
            if let Some(runtime) = self.recovery.as_mut() {
                invalidated += runtime.invalidate_path_edge(*left, *right);
                disrupted += runtime.disrupt_sessions_for_edge(*left, *right);
            }
        }
        self.add_ledger(cause, "cache-invalidated", invalidated, evidence);
        self.add_ledger(cause, "sessions-disrupted", disrupted, evidence);
        let orphans = self
            .graph
            .node_ids()
            .filter(|node| self.graph.is_active(*node))
            .filter(|node| {
                self.graph.parent(*node).is_some_and(|parent| {
                    self.graph
                        .edge_between(*node, parent)
                        .is_none_or(|edge| !self.graph.is_edge_active(edge))
                })
            })
            .collect::<Vec<_>>();
        for node in orphans {
            if let Some(transition) = self.evaluate_parent(node)? {
                self.apply_transition(node, transition, cause, Some(event), evidence)?;
            } else {
                self.graph.reset_self_root(node)?;
                self.root_generations.insert(self.graph.address(node)?);
                self.request_all(node, cause, Some(event))?;
                self.request_bloom_all(node, cause, Some(event))?;
            }
        }
        Ok(())
    }

    fn reconnect_edges(
        &mut self,
        event: EventId,
        cause: &str,
        edges: &[(EdgeId, NodeId, NodeId)],
    ) -> Result<(), RunError> {
        for (_, left, right) in edges {
            if self.graph.is_active(*left) && self.graph.is_active(*right) {
                self.request_announce(*left, *right, cause, Some(event))?;
                self.request_announce(*right, *left, cause, Some(event))?;
                self.request_bloom_all(*left, cause, Some(event))?;
                self.request_bloom_all(*right, cause, Some(event))?;
            }
        }
        Ok(())
    }

    pub(super) fn handle_link_update(
        &mut self,
        _event: EventId,
        evidence: &str,
        input: LinkUpdateInput,
    ) -> Result<Value, RunError> {
        let (from, to) = self.graph.edge(input.edge)?;
        let before = self.links.config(input.edge)?.clone();
        let mut after = before.clone();
        if input.restore {
            self.media_zones.configure_edge(
                input.edge,
                from,
                to,
                &self.transports,
                &mut self.links,
            )?;
            after = self.links.config(input.edge)?.clone();
        }
        if let Some(value) = input.bandwidth_bps {
            after.bandwidth_bps = value.max(1);
        }
        if let Some(value) = input.latency_ns {
            after.latency_ns = value;
        }
        if let Some(value) = input.jitter_ns {
            after.jitter_ns = value;
        }
        if let Some(value) = input.loss_ppm {
            after.loss_ppm = value;
        }
        if let Some(value) = input.mtu_bytes {
            after.mtu_bytes = value;
        }
        if let Some(value) = input.queue_bytes {
            after.queue_bytes = value;
        }
        self.links.set_config(input.edge, after.clone())?;
        let cause = format!("input:{}", input.id);
        self.add_ledger(&cause, "performed", 1, evidence);
        Ok(json!({
            "intervention_id": input.id, "edge": input.edge, "from": from, "to": to,
            "restored": input.restore, "before": before, "after": after
        }))
    }
}
