use super::*;

impl Simulation {
    pub(crate) fn handle_bloom_due(
        &mut self,
        event: EventId,
        evidence: &str,
        from: NodeId,
        to: NodeId,
        cause: &str,
    ) -> Result<Value, RunError> {
        let pending = self.bloom.as_mut().unwrap().pending.remove(&(from, to));
        if pending.as_ref().map(|item| item.event_id) != Some(event)
            || pending.as_ref().map(|item| item.cause.as_str()) != Some(cause)
        {
            return Err(RunError::Invariant(format!(
                "orphaned Bloom event {event} for {from}->{to}"
            )));
        }
        self.bloom.as_mut().unwrap().counters.wave.constructed += 1;
        if !self.graph.is_active(from) || !self.graph.is_active(to) {
            self.reject_bloom(
                cause,
                FILTER_ANNOUNCE_FMP_BYTES,
                "inactive endpoint",
                evidence,
            );
            return Ok(json!({"from": from, "to": to, "rejected": "inactive endpoint"}));
        }
        let edge = self.graph.edge_between(from, to).ok_or_else(|| {
            RunError::Invariant(format!("Bloom replacement has no edge {from}->{to}"))
        })?;
        if !self.graph.is_edge_active(edge) {
            self.reject_bloom(
                cause,
                FILTER_ANNOUNCE_FMP_BYTES,
                "partitioned edge",
                evidence,
            );
            return Ok(
                json!({"from": from, "to": to, "edge": edge, "rejected": "partitioned edge"}),
            );
        }
        let snapshot = self.bloom_snapshot(from, to)?;
        let maximum_fpr = self.bloom.as_ref().unwrap().max_fpr;
        if snapshot.fpr_ppb as f64 / 1_000_000_000.0 > maximum_fpr {
            self.reject_bloom(
                cause,
                FILTER_ANNOUNCE_FMP_BYTES,
                "antipoison FPR cap",
                evidence,
            );
            return Ok(bloom_json(
                from,
                to,
                &snapshot,
                None,
                Some("antipoison FPR cap"),
            ));
        }
        let link = self.links.config(edge)?.clone();
        let request = EnqueueRequest {
            edge_id: edge,
            from,
            to,
            class: LinkClass::Control,
            frame_bytes: FILTER_ANNOUNCE_FMP_BYTES,
            useful_payload_bytes: 0,
            now_ns: self.scheduler.now_ns(),
        };
        match self.links.enqueue(request) {
            Ok(result) => {
                let role = self.bloom_peer_role(from, to);
                let runtime = self.bloom.as_mut().unwrap();
                runtime.counters.wave.sent += 1;
                runtime.counters.wave.message_bytes += FILTER_ANNOUNCE_BYTES;
                runtime.counters.wave.fmp_bytes += FILTER_ANNOUNCE_FMP_BYTES;
                *runtime.counters.wave.by_role.entry(role).or_default() += 1;
                runtime.counters.transmitted_wire_bytes += result.transmitted_bytes;
                runtime.counters.lost_wire_bytes += result.lost_bytes;
                runtime
                    .last_sent_ns
                    .insert((from, to), self.scheduler.now_ns());
                self.add_ledger(cause, "constructed", 1, evidence);
                self.add_ledger(cause, "serialized", FILTER_ANNOUNCE_FMP_BYTES, evidence);
                self.add_ledger(cause, "queued", FILTER_ANNOUNCE_FMP_BYTES, evidence);
                self.add_ledger(cause, "transmitted", result.transmitted_bytes, evidence);
                let deliveries = result
                    .deliveries
                    .iter()
                    .map(|delivery| {
                        json!({
                            "deliver_at_ns": delivery.deliver_at_ns,
                            "copy": delivery.copy_ordinal
                        })
                    })
                    .collect::<Vec<_>>();
                for delivery in result.deliveries {
                    self.scheduler.schedule_at(
                        delivery.deliver_at_ns,
                        Some(event),
                        SimEvent::DeliverBloom {
                            delivery,
                            snapshot: snapshot.clone(),
                            cause: cause.to_owned(),
                        },
                    )?;
                }
                let mut value = bloom_json(from, to, &snapshot, Some(edge), None);
                let object = value.as_object_mut().unwrap();
                object.insert("role".to_owned(), json!(role));
                object.insert("frame_bytes".to_owned(), json!(FILTER_ANNOUNCE_FMP_BYTES));
                object.insert(
                    "transport_bytes".to_owned(),
                    json!(result.transmitted_bytes),
                );
                object.insert("lost_bytes".to_owned(), json!(result.lost_bytes));
                object.insert(
                    "queue_occupancy_bytes".to_owned(),
                    json!(result.queue_occupancy_bytes),
                );
                object.insert("bandwidth_bps".to_owned(), json!(link.bandwidth_bps));
                object.insert("latency_ns".to_owned(), json!(link.latency_ns));
                object.insert("mtu_bytes".to_owned(), json!(link.mtu_bytes));
                object.insert(
                    "from_transport".to_owned(),
                    json!(self.transports.profile(from).name),
                );
                object.insert(
                    "to_transport".to_owned(),
                    json!(self.transports.profile(to).name),
                );
                object.insert("deliveries".to_owned(), json!(deliveries));
                Ok(value)
            }
            Err(error @ (LinkError::MtuExceeded { .. } | LinkError::QueueFull { .. })) => {
                self.reject_bloom(
                    cause,
                    FILTER_ANNOUNCE_FMP_BYTES,
                    &error.to_string(),
                    evidence,
                );
                let mut value =
                    bloom_json(from, to, &snapshot, Some(edge), Some(&error.to_string()));
                let object = value.as_object_mut().unwrap();
                object.insert("frame_bytes".to_owned(), json!(FILTER_ANNOUNCE_FMP_BYTES));
                object.insert("bandwidth_bps".to_owned(), json!(link.bandwidth_bps));
                object.insert("latency_ns".to_owned(), json!(link.latency_ns));
                object.insert("mtu_bytes".to_owned(), json!(link.mtu_bytes));
                object.insert(
                    "from_transport".to_owned(),
                    json!(self.transports.profile(from).name),
                );
                object.insert(
                    "to_transport".to_owned(),
                    json!(self.transports.profile(to).name),
                );
                Ok(value)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn handle_bloom_delivery(
        &mut self,
        event: EventId,
        evidence: &str,
        delivery: &Delivery,
        snapshot: BloomSnapshot,
        cause: &str,
    ) -> Result<Value, RunError> {
        self.links.record_delivery(delivery, 0)?;
        let fresh = {
            let runtime = self.bloom.as_mut().unwrap();
            runtime.counters.delivered_frames += 1;
            runtime.counters.delivered_wire_bytes += delivery.wire_bytes;
            let fresh = self.graph.is_active(delivery.to)
                && runtime.peer_views.get(&(delivery.to, delivery.from)) != Some(&snapshot.model);
            if fresh {
                runtime
                    .peer_views
                    .insert((delivery.to, delivery.from), snapshot.model.clone());
            }
            fresh
        };
        self.add_ledger(cause, "delivered", delivery.wire_bytes, evidence);
        if fresh {
            self.request_bloom_except(delivery.to, Some(delivery.from), cause, Some(event))?;
        }
        Ok(json!({
            "edge": delivery.edge_id, "from": delivery.from, "to": delivery.to,
            "copy": delivery.copy_ordinal, "frame_bytes": delivery.frame_bytes,
            "wire_bytes": delivery.wire_bytes, "fresh": fresh,
            "occupied_bits": snapshot.occupied_bits, "fpr_ppb": snapshot.fpr_ppb,
            "estimated_cardinality": snapshot.estimated_cardinality
        }))
    }

    fn bloom_snapshot(&mut self, from: NodeId, to: NodeId) -> Result<BloomSnapshot, RunError> {
        // FIPS tree-only merge rule (fips-bloom-filters.md §"Filter Computation",
        // lines 133-144): only the filters of TREE peers (spanning-tree parent and
        // children) are folded into an outgoing filter. Non-tree mesh peers still
        // receive/store filters for routing but MUST NOT be merged transitively,
        // otherwise mesh shortcuts saturate every node's filter toward the full
        // network and destroy the upward=subtree / downward=complement asymmetry.
        // Split-horizon (fips-bloom-filters.md §"Split-Horizon Exclusion", lines
        // 147-154) still excludes the destination peer `to`.
        let tree_peers = self
            .graph
            .active_neighbors(from)
            .into_iter()
            .filter(|&peer| peer != to && self.bloom_is_tree_peer(from, peer))
            .collect::<Vec<_>>();
        let runtime = self.bloom.as_mut().unwrap();
        let mut model = runtime.local[from as usize].clone();
        for peer in tree_peers {
            if let Some(view) = runtime.peer_views.get(&(from, peer)) {
                runtime.counters.wave.bitwise_operations += model
                    .union_assign(view)
                    .map_err(|error| RunError::Invariant(error.to_string()))?;
            }
        }
        let occupied_bits = model.occupied_bits();
        let fpr_ppb = (model.fpr() * 1_000_000_000.0).round() as u64;
        let estimated_cardinality = model
            .estimated_cardinality(runtime.max_fpr)
            .map(|value| value.round() as u64);
        Ok(BloomSnapshot {
            model,
            occupied_bits,
            fpr_ppb,
            estimated_cardinality,
        })
    }

    fn bloom_peer_role(&self, from: NodeId, to: NodeId) -> PeerRole {
        if self.graph.parent(from) == Some(to) {
            PeerRole::Parent
        } else if self.graph.parent(to) == Some(from) {
            PeerRole::Child
        } else {
            PeerRole::Mesh
        }
    }

    /// A peer is a spanning-tree neighbor when it is `node`'s parent or one of
    /// `node`'s children — i.e. exactly the non-`Mesh` roles classified by
    /// [`Self::bloom_peer_role`]. This is the tree-only-merge membership test
    /// mandated by FIPS (fips-bloom-filters.md lines 133-144).
    fn bloom_is_tree_peer(&self, node: NodeId, peer: NodeId) -> bool {
        !matches!(self.bloom_peer_role(node, peer), PeerRole::Mesh)
    }

    fn reject_bloom(&mut self, cause: &str, bytes: u64, reason: &str, evidence: &str) {
        self.bloom.as_mut().unwrap().counters.wave.rejected += 1;
        self.add_ledger(cause, "constructed", 1, evidence);
        self.add_ledger(cause, "rejected", bytes, reason);
    }
}

fn bloom_json(
    from: NodeId,
    to: NodeId,
    snapshot: &BloomSnapshot,
    edge: Option<crate::EdgeId>,
    rejected: Option<&str>,
) -> Value {
    json!({
        "message": "filter-announce", "from": from, "to": to, "edge": edge,
        "occupied_bits": snapshot.occupied_bits, "fpr_ppb": snapshot.fpr_ppb,
        "estimated_cardinality": snapshot.estimated_cardinality, "rejected": rejected
    })
}
