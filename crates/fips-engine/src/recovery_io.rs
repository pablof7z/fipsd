use super::*;

impl<'a> CoupledState<'a> {
    pub(super) fn enqueue_new_bloom(&mut self, index: &mut usize) -> Result<(), RecoveryError> {
        while *index < self.bloom_wave.transmissions.len() {
            let transmission = self.bloom_wave.transmissions[*index].clone();
            self.enqueue_frame(
                &transmission.causal_id,
                None,
                "filter-announce",
                LinkClass::Control,
                transmission.frame_bytes,
                0,
                transmission.at_ns,
                0,
            )?;
            if let Some(summary) = self
                .arrival_summaries
                .iter_mut()
                .find(|summary| summary.causal_id == transmission.causal_id)
            {
                summary.bloom_frames = summary.bloom_frames.saturating_add(1);
                summary.bloom_fmp_bytes = summary
                    .bloom_fmp_bytes
                    .saturating_add(transmission.frame_bytes);
            }
            *index += 1;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn enqueue_frame(
        &mut self,
        cause: &str,
        parent: Option<&str>,
        message: &str,
        class: LinkClass,
        frame_bytes: u64,
        useful_bytes: u64,
        now_ns: u64,
        depth: u32,
    ) -> Result<u64, RecoveryError> {
        self.maximum_frame_bytes = self.maximum_frame_bytes.max(frame_bytes);
        match self.link.enqueue(EnqueueRequest {
            edge_id: 0,
            from: 0,
            to: 1,
            class,
            frame_bytes,
            useful_payload_bytes: useful_bytes,
            now_ns,
        }) {
            Ok(result) => {
                let wire_bytes = frame_bytes + self.config.link.transport_overhead_bytes;
                let duplicate_bytes = result.transmitted_bytes.saturating_sub(wire_bytes);
                self.accepted_frame_bytes = self.accepted_frame_bytes.saturating_add(frame_bytes);
                self.logical_wire_bytes = self
                    .logical_wire_bytes
                    .saturating_add(frame_bytes + self.config.link.transport_overhead_bytes);
                *self
                    .projections
                    .by_message
                    .entry(message.to_owned())
                    .or_default() += frame_bytes;
                *self
                    .projections
                    .by_depth_band
                    .entry(depth_band(depth).to_owned())
                    .or_default() += frame_bytes;
                self.ledger.push(ledger_child(
                    cause,
                    parent,
                    "serialized",
                    frame_bytes,
                    message,
                ));
                self.ledger
                    .push(ledger_child(cause, parent, "fmp", frame_bytes, message));
                if message != "filter-announce" && message != "session-data" {
                    self.ledger.push(ledger_child(
                        cause,
                        parent,
                        "fsp",
                        frame_bytes.saturating_sub(ESTABLISHED_FMP_OVERHEAD_BYTES),
                        message,
                    ));
                }
                self.ledger.push(ledger_child(
                    cause,
                    parent,
                    "transport",
                    self.config.link.transport_overhead_bytes,
                    "configured-link-overhead",
                ));
                self.ledger
                    .push(ledger_child(cause, parent, "queued", wire_bytes, "edge:0"));
                self.ledger.push(ledger_child(
                    cause,
                    parent,
                    "transmitted",
                    result.transmitted_bytes,
                    "edge:0",
                ));
                self.ledger.push(ledger_child(
                    cause,
                    parent,
                    "network",
                    result.transmitted_bytes,
                    "edge:0",
                ));
                self.ledger.push(ledger_child(
                    cause,
                    parent,
                    "lower-bound",
                    wire_bytes,
                    "one-logical-copy",
                ));
                let reliability = result.lost_bytes.saturating_add(duplicate_bytes);
                if reliability > 0 {
                    self.ledger.push(ledger_child(
                        cause,
                        parent,
                        "reliability",
                        reliability,
                        "loss-plus-duplicate",
                    ));
                }
                if duplicate_bytes > 0 {
                    self.ledger.push(ledger_child(
                        cause,
                        parent,
                        "duplicate",
                        duplicate_bytes,
                        "deterministic-link-copy",
                    ));
                    self.ledger.push(ledger_child(
                        cause,
                        parent,
                        "retransmitted",
                        duplicate_bytes,
                        "deterministic-link-copy",
                    ));
                }
                let mut delivered_useful = 0_u64;
                for delivery in result.deliveries {
                    self.link_delivery_ns = self.link_delivery_ns.max(delivery.deliver_at_ns);
                    let credited_useful = if delivery.class == LinkClass::UsefulPayload
                        && delivery.copy_ordinal == 0
                    {
                        useful_bytes
                    } else {
                        0
                    };
                    delivered_useful = delivered_useful.saturating_add(credited_useful);
                    self.link.record_delivery(&delivery, credited_useful)?;
                    self.ledger.push(ledger_child(
                        cause,
                        parent,
                        "delivered",
                        delivery.wire_bytes,
                        &format!("edge:0:copy-{}", delivery.copy_ordinal),
                    ));
                    if credited_useful > 0 {
                        self.ledger.push(ledger_child(
                            cause,
                            parent,
                            "useful-payload",
                            credited_useful,
                            message,
                        ));
                    }
                }
                Ok(delivered_useful)
            }
            Err(
                error @ (crate::LinkError::MtuExceeded { .. } | crate::LinkError::QueueFull { .. }),
            ) => {
                self.ledger.push(ledger_child(
                    cause,
                    parent,
                    "rejected",
                    frame_bytes,
                    &error.to_string(),
                ));
                Ok(0)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn consume(
        &mut self,
        node: u32,
        cause: &str,
        kind: ResourceKind,
        units: u64,
        at_ns: u64,
    ) {
        match self.resources[node as usize].consume(cause, node, kind, units, at_ns) {
            Ok(receipt) => self.ledger.push(ledger(
                cause,
                if kind.is_service() {
                    "compute"
                } else {
                    "state-mutated"
                },
                units,
                &format!("resource:{kind:?}:done-{}", receipt.completed_at_ns),
            )),
            Err(error) => {
                self.ledger
                    .push(ledger(cause, "rejected", units, &error.to_string()));
                self.resource_exhaustions.push(error);
            }
        }
    }

    pub(super) fn starve(&mut self, useful_bytes: u64) {
        self.traffic.starved_flows += 1;
        self.traffic.lost_useful_bytes =
            self.traffic.lost_useful_bytes.saturating_add(useful_bytes);
    }
}
