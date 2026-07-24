import Foundation

enum CampaignIdentityBudget {
    static func adjustScale(
        in campaign: inout [String: Any],
        previousManualArrivals: Int,
        currentManualArrivals: Int,
        newAttachmentTargets: [[Int]]
    ) {
        guard var scale = campaign["scale"] as? [String: Any],
              let nodes = scale["nodes"] as? Int else { return }
        let updatedNodes = max(2, nodes + currentManualArrivals - previousManualArrivals)
        scale["nodes"] = updatedNodes
        campaign["scale"] = scale
        guard var topology = campaign["topology"] as? [String: Any],
              topology["generator"] as? String == "explicit",
              var edges = topology["explicit_edges"] as? [[Int]] else { return }
        if updatedNodes < nodes {
            edges.removeAll { $0.contains { $0 >= updatedNodes } }
        } else if updatedNodes > nodes {
            for (offset, node) in (nodes..<updatedNodes).enumerated() {
                let targets = newAttachmentTargets[safe: offset] ?? [0]
                for rawTarget in targets {
                    let target = min(max(0, rawTarget), node - 1)
                    edges.append([target, node])
                }
            }
        }
        topology["explicit_edges"] = edges
        campaign["topology"] = topology
    }

    static func reserveManualArrivals(in campaign: inout [String: Any]) {
        guard var identities = campaign["identities"] as? [String: Any],
              var arrivals = identities["arrivals"] as? [String: Any] else { return }
        let scheduled = arrivals["count"] as? Int ?? 0
        let events = campaign["events"] as? [[String: Any]] ?? []
        let manual = events.filter {
            ["introduce-lower-root-node", "introduce-node"].contains(
                $0["action"] as? String
            )
        }.count
        let required = scheduled + manual
        var budget = arrivals["attacker_budget"] as? [String: Any] ?? [:]
        budget["mode"] = "bounded"
        budget["identities"] = max(budget["identities"] as? Int ?? 0, required)
        budget["operations"] = max(budget["operations"] as? Int ?? 0, required)
        arrivals["attacker_budget"] = budget
        identities["arrivals"] = arrivals
        campaign["identities"] = identities
    }
}

private extension Array {
    subscript(safe index: Index) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
