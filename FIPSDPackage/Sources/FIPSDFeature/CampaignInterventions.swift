import Foundation

struct NetworkIntervention: Equatable, Sendable {
    let atSeconds: Double
    let action: String
    let nodes: [Int]
}

struct LinkIntervention: Equatable, Sendable {
    let atSeconds: Double
    let action: String
    let edge: Int
    var bandwidthMbps: Int?
    var latencyMilliseconds: Double?
    var jitterMilliseconds: Double? = nil
    var lossPPM: Int?
    var mtuBytes: Int?
}

struct LookupStormIntervention: Equatable, Sendable {
    let atSeconds: Double
    let count: Int
}

struct TransportClassIntervention: Equatable, Sendable {
    let atSeconds: Double
    let profile: String
    let restore: Bool
}

struct ParentIntervention: Equatable, Sendable {
    let atSeconds: Double
    let action: String
    let node: Int?
    let cycles: Int
}

struct SybilIntervention: Equatable, Sendable {
    let atSeconds: Double
    let count: Int
    let intervalMilliseconds: Int
    let attachment: String
    let rootGrinding: Bool
}

extension CampaignBuilder {
    static func interventionEvents(
        _ raw: CampaignConfiguration,
        nodes: Int,
        reserved: Int,
        manualRootCount: Int,
        sybilLimit: Int
    ) -> [[String: Any]] {
        let lifecycle = raw.lifecycleEvents.enumerated().map { index, event in
            [
                "id": "lifecycle-\(index)",
                "at": duration(seconds: event.atSeconds),
                "action": event.action,
                "target": min(max(0, event.node), max(0, nodes - reserved - 1))
            ] as [String: Any]
        }
        let roots = raw.manualRootTimes.prefix(manualRootCount).enumerated().map { index, time in
            [
                "id": "manual-root-\(index)",
                "at": duration(seconds: time),
                "action": "introduce-lower-root-node"
            ] as [String: Any]
        }
        let networks = raw.networkEvents.enumerated().map { index, event in
            [
                "id": "network-\(index)",
                "at": duration(seconds: event.atSeconds),
                "action": event.action,
                "parameters": ["nodes": event.nodes]
            ] as [String: Any]
        }
        let rekeys = raw.sessionRekeyTimes.enumerated().map { index, time in
            [
                "id": "session-rekey-\(index)",
                "at": duration(seconds: time),
                "action": "synchronized-session-rekey"
            ] as [String: Any]
        }
        let lookupStorms = raw.lookupStorms.enumerated().flatMap { index, storm in
            [
                [
                    "id": "cache-expiry-\(index)",
                    "at": duration(seconds: storm.atSeconds),
                    "action": "expire-coordinate-cache"
                ],
                [
                    "id": "lookup-wave-\(index)",
                    "at": duration(seconds: storm.atSeconds),
                    "action": "simultaneous-lookups",
                    "parameters": ["count": min(max(1, storm.count), 100_000)]
                ]
            ] as [[String: Any]]
        }
        let transportClasses = raw.transportClassEvents.enumerated().map { index, event in
            [
                "id": "transport-class-\(index)",
                "at": duration(seconds: event.atSeconds),
                "action": event.restore
                    ? "restore-transport-class"
                    : "fail-transport-class",
                "target": event.profile
            ] as [String: Any]
        }
        let parents = raw.parentEvents.enumerated().map { index, event in
            var value: [String: Any] = [
                "id": "parent-\(index)",
                "at": duration(seconds: event.atSeconds),
                "action": event.action,
                "parameters": [
                    "cycles": min(max(1, event.cycles), 1_000),
                    "interval": duration(
                        milliseconds: Double(raw.parentOscillationIntervalMilliseconds)
                    ),
                    "preferred_cost_ppm": 1_000_000,
                    "degraded_cost_ppm": 6_000_000
                ]
            ]
            if let node = event.node { value["target"] = min(max(0, node), nodes - 1) }
            return value
        }
        var remainingSybils = sybilLimit
        let sybils: [[String: Any]] = raw.sybilEvents.enumerated().compactMap {
            index, event -> [String: Any]? in
            let count = min(min(max(1, event.count), 100_000), remainingSybils)
            guard count > 0 else { return nil }
            remainingSybils -= count
            return [
                "id": "sybil-\(index)",
                "at": duration(seconds: event.atSeconds),
                "action": "attach-authenticated-sybils",
                "parameters": [
                    "count": count,
                    "interval": duration(
                        milliseconds: Double(max(0, event.intervalMilliseconds))
                    ),
                    "attachment": event.attachment,
                    "address_policy":
                        event.rootGrinding ? "lower-than-current-root" : "uniform-valid",
                    "operations_per_identity": 1
                ]
            ] as [String: Any]
        }
        let links = raw.linkEvents.enumerated().map { index, event in
            var parameters: [String: Any] = [:]
            if let value = event.bandwidthMbps {
                parameters["bandwidth_bps"] = max(1, value) * 1_000_000
            }
            if let value = event.latencyMilliseconds {
                parameters["latency"] = duration(milliseconds: value)
            }
            if let value = event.jitterMilliseconds {
                parameters["jitter"] = duration(milliseconds: value)
            }
            if let value = event.lossPPM {
                parameters["loss_ppm"] = min(max(0, value), 1_000_000)
            }
            if let value = event.mtuBytes { parameters["mtu_bytes"] = max(68, value) }
            return [
                "id": "link-\(index)",
                "at": duration(seconds: event.atSeconds),
                "action": event.action,
                "target": max(0, event.edge),
                "parameters": parameters
            ] as [String: Any]
        }
        return [[
            "id": "descending-root-arrivals",
            "action": "introduce-lower-root-identities"
        ]] + lifecycle + roots + rekeys + lookupStorms + transportClasses
            + parents + sybils + networks + links
    }
}
