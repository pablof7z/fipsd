use super::*;

impl Simulation {
    pub(super) fn topology_snapshot(&self) -> Result<Value, RunError> {
        let nodes = self
            .graph
            .node_ids()
            .map(|id| {
                Ok(json!({
                    "id": id,
                    "address": self.graph.address(id)?.to_hex(),
                    "active": self.graph.is_active(id),
                    "root": self.graph.root(id),
                    "parent": self.graph.parent(id),
                    "sequence": self.graph.sequence(id),
                    "transport_profile": self.transports.profile(id).name,
                    "transport_type": format!("{:?}", self.transports.profile(id).media.kind).to_lowercase(),
                    "bandwidth_bps": self.transports.profile(id).media.bandwidth_bps,
                    "latency_ns": self.transports.profile(id).media.latency_ns,
                    "jitter_ns": self.transports.profile(id).media.jitter_ns,
                    "mtu_bytes": self.transports.profile(id).media.effective_mtu_bytes
                    ,"media_zone": self.media_zones.zone_id(id)
                }))
            })
            .collect::<Result<Vec<_>, RunError>>()?;
        let edges = (0..self.graph.edge_count())
            .map(|id| {
                let (from, to) = self.graph.edge(id as u32)?;
                let link = self.links.config(id as u32)?;
                Ok(json!({
                    "id": id, "from": from, "to": to,
                    "active": self.graph.is_edge_active(id as u32),
                    "bandwidth_bps": link.bandwidth_bps,
                    "latency_ns": link.latency_ns,
                    "jitter_ns": link.jitter_ns,
                    "loss_ppm": link.loss_ppm,
                    "mtu_bytes": link.mtu_bytes,
                    "queue_bytes": link.queue_bytes
                    ,"shared_medium_group": self.links.shared_group(id as u32),
                    "parent_cost_ppm": self.parent_cost_ppm[id]
                }))
            })
            .collect::<Result<Vec<_>, RunError>>()?;
        Ok(json!({
            "active_nodes": nodes.iter().filter(|node| node["active"] == true).count(),
            "nodes": nodes,
            "edges": edges
        }))
    }
}
