import Foundation

struct LiveRunContext: Sendable {
    let data: Data
    let realizedArrivals: Int
    let currentRootID: Int

    static func make(
        state: SimulationState,
        events: [SimulationEvent],
        cursor: Int,
        timeNS: UInt64,
        campaign: Data
    ) throws -> Self {
        let rendered = events.prefix(min(cursor, events.count))
        var joinedAt = Dictionary(uniqueKeysWithValues: state.nodes.keys.map { ($0, UInt64(0)) })
        var realizedArrivals = 0
        for event in rendered where [
            "input.descending-root-arrival",
            "input.node-arrived",
            "input.authenticated-sybil-arrived"
        ].contains(event.kind) {
            guard let node = event.data["node"]?.int else { continue }
            joinedAt[node] = event.timeNS
            realizedArrivals += 1
        }
        let nodes = state.nodes.values.sorted { $0.id < $1.id }.map { node in
            [
                "id": node.id,
                "label": label(for: node.id),
                "address": node.address,
                "active": node.active,
                "root": node.root,
                "parent": node.parent.map { $0 as Any } ?? NSNull(),
                "joined_at_ns": joinedAt[node.id] ?? 0,
                "transport_profile": node.transportProfile,
                "transport_type": node.transportType,
                "bandwidth_bps": node.bandwidthBPS,
                "latency_ns": node.latencyNS,
                "mtu_bytes": node.mtuBytes
            ] as [String: Any]
        }
        let edges = state.edges.values.sorted { $0.id < $1.id }.map { edge in
            [
                "id": edge.id,
                "from": edge.from,
                "to": edge.to,
                "active": edge.active,
                "bandwidth_bps": edge.bandwidthBPS,
                "latency_ns": edge.latencyNS,
                "jitter_ns": edge.jitterNS,
                "loss_ppm": edge.lossPPM,
                "mtu_bytes": edge.mtuBytes,
                "queue_bytes": edge.queueBytes
            ] as [String: Any]
        }
        let transfers = state.applicationTransfers.values.sorted { $0.id < $1.id }.map {
            transfer in
            [
                "id": transfer.id,
                "source": transfer.source,
                "destination": transfer.destination,
                "path": transfer.path,
                "total_bytes": transfer.totalBytes,
                "offered_bytes": transfer.offeredBytes,
                "delivered_bytes": transfer.deliveredBytes,
                "progress": transfer.progress
            ] as [String: Any]
        }
        let campaignObject = try JSONSerialization.jsonObject(with: campaign) as? [String: Any]
        let currentRootID = state.nodes.values
            .filter(\.active)
            .min { $0.address < $1.address }?
            .id ?? 0
        var context: [String: Any] = [
            "cursor_virtual_time_ns": timeNS,
            "current_root_node": currentRootID,
            "rendered_event_cursor": cursor,
            "known_event_count": events.count,
            "active_node_count": nodes.filter { $0["active"] as? Bool == true }.count,
            "realized_arrival_count": realizedArrivals,
            "nodes_oldest_first": nodes.sorted {
                ($0["joined_at_ns"] as? UInt64 ?? 0) < ($1["joined_at_ns"] as? UInt64 ?? 0)
            },
            "edges": edges,
            "application_transfers": transfers,
            "scheduled_campaign_events": campaignObject?["events"] ?? []
        ]
        if let last = state.lastEvent {
            context["last_rendered_event"] = [
                "id": last.id, "kind": last.kind, "virtual_time_ns": last.timeNS
            ]
        }
        return Self(
            data: try JSONSerialization.data(
                withJSONObject: context,
                options: [.prettyPrinted, .sortedKeys]
            ),
            realizedArrivals: realizedArrivals,
            currentRootID: currentRootID
        )
    }

    private static func label(for id: Int) -> String {
        guard (0..<26).contains(id),
              let scalar = UnicodeScalar(Int(UnicodeScalar("a").value) + id) else {
            return "#\(id)"
        }
        return String(Character(scalar))
    }
}
