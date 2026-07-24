import Foundation

struct DiagnosticBucket: Identifiable, Equatable, Sendable {
    var id: String { label }
    let label: String
    let count: UInt64
}

struct CongestionCell: Identifiable, Equatable, Sendable {
    var id: String { "\(from)->\(to)" }
    let from: String
    let to: String
    let bytes: UInt64
    let frames: UInt64
}

struct ArtifactDiagnostics: Equatable, Sendable {
    var latencyP50NS: UInt64 = 0
    var latencyP95NS: UInt64 = 0
    var latencyP99NS: UInt64 = 0
    var deliverySamples = 0
    var bloomFPRP50PPB: UInt64 = 0
    var bloomFPRP95PPB: UInt64 = 0
    var bloomFPRP99PPB: UInt64 = 0
    var bloomFPRSamples = 0
    var queueHistogram: [DiagnosticBucket] = []
    var planeLoads: [DiagnosticBucket] = []
    var congestion: [CongestionCell] = []

    static func build(_ events: [SimulationEvent]) -> ArtifactDiagnostics {
        let topology = events.first { $0.kind == "input.initial-topology" }
        let groups = nodeGroups(topology)
        var latencies: [UInt64] = []
        var queues: [UInt64] = []
        var bloomFPR: [UInt64] = []
        var planes: [String: UInt64] = [:]
        var cells: [String: (String, String, UInt64, UInt64)] = [:]

        for event in events where isDue(event.kind) {
            let bytes = event.data["transport_bytes"]?.uint64
                ?? event.data["frame_bytes"]?.uint64 ?? 0
            planes[plane(event.kind), default: 0] += bytes
            if let queue = event.data["queue_occupancy_bytes"]?.uint64 { queues.append(queue) }
            for delivery in event.data["deliveries"]?.array ?? [] {
                if let time = delivery.object?["deliver_at_ns"]?.uint64, time >= event.timeNS {
                    latencies.append(time - event.timeNS)
                }
            }
            guard let from = event.data["from"]?.int, let to = event.data["to"]?.int else { continue }
            let left = groups[from] ?? "ungrouped"
            let right = groups[to] ?? "ungrouped"
            let key = "\(left)->\(right)"
            let old = cells[key, default: (left, right, 0, 0)]
            cells[key] = (left, right, old.2 + bytes, old.3 + 1)
        }
        for event in events where event.kind == "bloom.filter-delivered" {
            if let value = event.data["fpr_ppb"]?.uint64 { bloomFPR.append(value) }
        }
        latencies.sort()
        bloomFPR.sort()
        return ArtifactDiagnostics(
            latencyP50NS: percentile(latencies, 50),
            latencyP95NS: percentile(latencies, 95),
            latencyP99NS: percentile(latencies, 99),
            deliverySamples: latencies.count,
            bloomFPRP50PPB: percentile(bloomFPR, 50),
            bloomFPRP95PPB: percentile(bloomFPR, 95),
            bloomFPRP99PPB: percentile(bloomFPR, 99),
            bloomFPRSamples: bloomFPR.count,
            queueHistogram: histogram(queues),
            planeLoads: planes.map { DiagnosticBucket(label: $0.key, count: $0.value) }
                .sorted { $0.count == $1.count ? $0.label < $1.label : $0.count > $1.count },
            congestion: cells.values.map {
                CongestionCell(from: $0.0, to: $0.1, bytes: $0.2, frames: $0.3)
            }.sorted { $0.bytes == $1.bytes ? $0.id < $1.id : $0.bytes > $1.bytes }
        )
    }

    private static func nodeGroups(_ event: SimulationEvent?) -> [Int: String] {
        Dictionary(uniqueKeysWithValues: (event?.data["nodes"]?.array ?? []).compactMap { item in
            guard let node = item.object, let id = node["id"]?.int else { return nil }
            return (id, node["media_zone"]?.string ?? node["transport_profile"]?.string ?? "node")
        })
    }

    private static func plane(_ kind: String) -> String {
        if kind.hasPrefix("data.") { return "payload" }
        if kind.hasPrefix("bloom.") { return "Bloom" }
        if kind.hasPrefix("lookup.") { return "lookup" }
        if kind.hasPrefix("session.") { return "session" }
        return "control"
    }

    private static func isDue(_ kind: String) -> Bool {
        kind.hasSuffix(".due") || kind.hasSuffix("-due")
    }

    private static func percentile(_ sorted: [UInt64], _ percentile: Int) -> UInt64 {
        guard !sorted.isEmpty else { return 0 }
        let rank = (sorted.count * percentile + 99) / 100
        let index = min(sorted.count - 1, max(0, rank - 1))
        return sorted[index]
    }

    private static func histogram(_ values: [UInt64]) -> [DiagnosticBucket] {
        let limits: [(String, UInt64)] = [
            ("≤1 KiB", 1 << 10), ("≤4 KiB", 4 << 10), ("≤16 KiB", 16 << 10),
            ("≤64 KiB", 64 << 10), (">64 KiB", .max)
        ]
        var counts = Array(repeating: UInt64(0), count: limits.count)
        for value in values {
            if let index = limits.firstIndex(where: { value <= $0.1 }) { counts[index] += 1 }
        }
        return zip(limits, counts).map { DiagnosticBucket(label: $0.0.0, count: $0.1) }
    }
}
