use super::*;

impl Simulation {
    pub(super) fn handle_sybil_arrival(
        &mut self,
        event: EventId,
        evidence: &str,
        input: SybilArrivalInput,
        node: NodeId,
    ) -> Result<Value, RunError> {
        if self.graph.is_active(node) {
            return Err(RunError::Invariant(format!(
                "authenticated Sybil slot {node} is already active"
            )));
        }
        let cause = format!("input:{}:sybil-{:08}", input.id, input.ordinal);
        self.add_ledger(&cause, "requested", 1, evidence);
        let previous_minimum = self.minimum_active_address()?;
        if input.address_policy == "lower-than-current-root" {
            let address = previous_minimum.one_lower().ok_or_else(|| {
                RunError::Invariant("cannot grind below the zero root".to_owned())
            })?;
            self.graph.set_address(node, address)?;
            self.identity_trials = self.identity_trials.saturating_add(input.operations);
            self.add_ledger(
                &cause,
                "identity-generation-operations",
                input.operations,
                evidence,
            );
        }
        let target = self.graph.select_attachment(
            input.attachment,
            self.plan.seed,
            u64::from(input.ordinal),
        )?;
        self.reset_local_bloom(node)?;
        self.graph.reset_self_root(node)?;
        self.graph.set_active(node, true)?;
        let edge = if let Some(edge) = self.graph.edge_between(node, target) {
            self.graph.set_edge_active(edge, true)?;
            edge
        } else {
            let edge = self.graph.add_edge(node, target)?;
            self.partition_blocks.push(0);
            self.parent_cost_ppm.push(1_000_000);
            self.media_zones.configure_edge(
                edge,
                node,
                target,
                &self.transports,
                &mut self.links,
            )?;
            edge
        };
        let address = self.graph.address(node)?;
        if address < previous_minimum {
            self.root_generations.insert(address);
        }
        self.authenticated_sybil_arrivals = self.authenticated_sybil_arrivals.saturating_add(1);
        self.add_ledger(&cause, "attacker-operations", input.operations, evidence);
        self.add_ledger(&cause, "authenticated-identities", 1, evidence);
        self.add_ledger(&cause, "signature-verifications", 1, evidence);
        self.add_ledger(&cause, "performed", 1, evidence);
        self.request_all(node, &cause, Some(event))?;
        self.request_announce(target, node, &cause, Some(event))?;
        self.request_bloom_all(node, &cause, Some(event))?;
        self.request_bloom_all(target, &cause, Some(event))?;
        Ok(json!({
            "intervention_id": input.id,
            "sybil_ordinal": input.ordinal,
            "node": node,
            "address": address.to_hex(),
            "active": true,
            "root": node,
            "parent": null,
            "sequence": self.graph.sequence(node),
            "authenticated": true,
            "malformed_wire": false,
            "address_policy": input.address_policy,
            "attachment": format!("{:?}", input.attachment),
            "target": target,
            "edge": edge,
            "attacker_operations": input.operations,
            "transport_profile": self.transports.profile(node).name,
            "bandwidth_bps": self.transports.profile(node).media.bandwidth_bps,
            "media_zone": self.media_zones.zone_id(node),
            "shared_medium_group": self.links.shared_group(edge)
        }))
    }
}
