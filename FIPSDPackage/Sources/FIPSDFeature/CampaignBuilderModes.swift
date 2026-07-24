import Foundation

extension CampaignBuilder {
    static func cohortData(for raw: CampaignConfiguration, nodes: Int) throws -> Data {
        guard var campaign = try JSONSerialization.jsonObject(with: data(for: raw)) as? [String: Any]
        else { throw CocoaError(.fileReadCorruptFile) }
        campaign["engine"] = [
            "modes": "cohort-analytical", "deterministic": true,
            "variant": "fips-80c956a-baseline"
        ]
        campaign["scale"] = ["nodes": nodes]
        if raw.topology == "explicit" {
            campaign["topology"] = ["generator": "balanced-tree", "average_degree": 4]
        } else if var topology = campaign["topology"] as? [String: Any] {
            topology.removeValue(forKey: "media_zones")
            campaign["topology"] = topology
        }
        campaign["transports"] = ["assignment": "heterogeneous"]
        if var identities = campaign["identities"] as? [String: Any],
           var arrivals = identities["arrivals"] as? [String: Any] {
            let count = min(32, max(0, raw.arrivals))
            arrivals["count"] = count
            arrivals["attacker_budget"] = [
                "mode": "bounded", "operations": count, "identities": count
            ]
            identities["arrivals"] = arrivals
            campaign["identities"] = identities
        }
        var approximations: [[String: String]] = [[
            "method": "bounded-depth-degree-cohorts/v1",
            "uncertainty": "deterministic per-metric bounds; billion-node scale is extrapolated",
            "validated_range": "2..=1000000 nodes where calibrated"
        ]]
        if raw.mediaZonesEnabled {
            approximations.append([
                "method": "shared-media-zone-aggregation/v1",
                "uncertainty": "individual intra-zone queue contention is aggregated by cohort",
                "validated_range": "individual engine only for exact queue order"
            ])
        }
        campaign["fidelity"] = [
            "protocol": "cohort", "serialization": "modeled",
            "bloom": "cohort-fpr", "crypto": "operation-count",
            "billion_node_representation": "cohort-with-sampled-exact-regions",
            "approximations": approximations
        ]
        return try JSONSerialization.data(
            withJSONObject: campaign,
            options: [.prettyPrinted, .sortedKeys]
        )
    }

    static func searchData(for raw: CampaignConfiguration) throws -> Data {
        guard var campaign = try JSONSerialization.jsonObject(with: data(for: raw)) as? [String: Any],
              var topology = campaign["topology"] as? [String: Any],
              var identities = campaign["identities"] as? [String: Any],
              var arrivals = identities["arrivals"] as? [String: Any],
              var schedule = arrivals["schedule"] as? [String: Any],
              var traffic = campaign["traffic"] as? [String: Any]
        else { throw CocoaError(.fileReadCorruptFile) }
        let upperNodes = min(24, max(4, raw.nodes))
        campaign["scale"] = [
            "nodes": ["values": raw.mediaZonesEnabled
                ? [upperNodes]
                : [max(4, upperNodes / 2), upperNodes]]
        ]
        let primaryTopology = raw.topology == "explicit" ? "chain" : raw.topology
        if raw.mediaZonesEnabled {
            topology = topologyConfiguration(raw, nodes: upperNodes)
        }
        topology["generator"] = ["values": Array(Set([primaryTopology, "chain"])).sorted()]
        topology.removeValue(forKey: "explicit_edges")
        campaign["topology"] = topology
        let intervalMS = max(1, raw.intervalSeconds * 1_000)
        schedule["interval"] = ["values": [
            duration(milliseconds: intervalMS * 0.998),
            duration(milliseconds: intervalMS * 1.002)
        ]]
        arrivals["attachment"] = ["values": Array(Set([raw.attachment, "hub"])).sorted()]
        arrivals["schedule"] = schedule
        arrivals["count"] = min(4, max(1, raw.arrivals))
        arrivals["attacker_budget"] = ["mode": "bounded", "operations": 4, "identities": 4]
        identities["arrivals"] = arrivals
        campaign["identities"] = identities
        traffic["model"] = ["values": Array(Set([raw.trafficModel, "session-churn"])).sorted()]
        traffic["parameters"] = ["flow_count": min(24, max(4, raw.trafficFlows))]
        campaign["traffic"] = traffic
        campaign["instrumentation"] = [
            "root_agreement_by_depth": true, "transition_stages": true,
            "causal_cost_ledger": true, "queue_wait": true,
            "control_and_useful_bytes": true,
            "quiescence_markers": ["root", "tree", "bloom", "lookup", "data-plane"]
        ]
        campaign["objectives"] = [
            "maximize": ["control_bytes_per_root_arrival", "data_plane_stall_duration"],
            "constraints": ["/scale/nodes <= 24"]
        ]
        return try JSONSerialization.data(
            withJSONObject: campaign,
            options: [.prettyPrinted, .sortedKeys]
        )
    }
}
