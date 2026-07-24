import Foundation

struct AnalysisMetric: Identifiable, Equatable, Sendable {
    var id: String { name }
    let name: String
    let unit: String
    let value: String
}

struct CausalStageSummary: Identifiable, Equatable, Sendable {
    var id: String { stage }
    let stage: String
    let count: UInt64
    let entries: Int
}

struct EdgeLoadSummary: Identifiable, Equatable, Sendable {
    var id: String { "\(from)-\(to)" }
    let from: Int
    let to: Int
    let frames: UInt64
    let bytes: UInt64
    let peakQueueBytes: UInt64
}

struct RootImpactSummary: Identifiable, Equatable, Sendable {
    let id: String
    let root: String
    let arrivalNS: UInt64
    let consequences: Int
}

struct CausalFlameSummary: Identifiable, Equatable, Sendable {
    let id: String
    let label: String
    let eventCount: UInt64
    let slices: [DiagnosticBucket]
}

struct ArtifactAnalysis: Equatable, Sendable {
    var representedNodes: UInt64 = 0
    var fidelity = "No analysis loaded"
    var metrics: [AnalysisMetric] = []
    var stages: [CausalStageSummary] = []
    var topEdges: [EdgeLoadSummary] = []
    var rootImpacts: [RootImpactSummary] = []
    var causalFlames: [CausalFlameSummary] = []
    var anomalyNodeIDs: Set<Int> = []
    var diagnostics = ArtifactDiagnostics()
    var longestCausalChain = 0
    var ledgerEntries = 0
    var eventCount = 0

    static func parse(_ root: [String: JSONValue]) -> ArtifactAnalysis {
        var result = ArtifactAnalysis()
        result.readManifest(root["manifest"]?.object)
        result.readMetrics(root["metric_series"]?.array ?? [])
        result.readLedger(root["causal_ledger"]?.array ?? [])
        result.readEvents(root["event_trace"]?.array ?? [])
        return result
    }

    private mutating func readManifest(_ manifest: [String: JSONValue]?) {
        guard let object = manifest?["fidelity"]?.object else { return }
        representedNodes = object["represented_nodes"]?.uint64 ?? 0
        let approximations = object["approximations"]?.array?.count ?? 0
        fidelity = [
            object["scale"]?.string ?? "unknown scale",
            object["protocol"]?.string ?? "unknown protocol",
            object["wire"]?.string ?? "unknown wire",
            approximations == 0 ? "no approximations" : "\(approximations) approximation labels"
        ].joined(separator: " · ")
    }

    private mutating func readMetrics(_ series: [JSONValue]) {
        metrics = series.compactMap { item in
            guard let object = item.object,
                  let name = object["name"]?.string,
                  let last = object["points"]?.array?.last?.object?["value"]?.string else { return nil }
            return AnalysisMetric(name: name, unit: object["unit"]?.string ?? "", value: last)
        }
    }

    private mutating func readLedger(_ entries: [JSONValue]) {
        var totals: [String: (UInt64, Int)] = [:]
        for entry in entries {
            guard let object = entry.object, let stage = object["stage"]?.string else { continue }
            let previous = totals[stage, default: (0, 0)]
            totals[stage] = (previous.0 + (object["count"]?.uint64 ?? 0), previous.1 + 1)
        }
        ledgerEntries = entries.count
        stages = totals.map { CausalStageSummary(stage: $0.key, count: $0.value.0, entries: $0.value.1) }
            .sorted { $0.count == $1.count ? $0.stage < $1.stage : $0.count > $1.count }
    }

    private mutating func readEvents(_ values: [JSONValue]) {
        let events = values.compactMap(SimulationEvent.init)
        eventCount = events.count
        longestCausalChain = Self.longestChain(events)
        rootImpacts = Self.rootImpacts(events)
        topEdges = Self.edgeLoads(events)
        causalFlames = Self.causalFlames(events)
        anomalyNodeIDs = Set(topEdges.prefix(12).flatMap { [$0.from, $0.to] })
        diagnostics = ArtifactDiagnostics.build(events)
    }

    private static func longestChain(_ events: [SimulationEvent]) -> Int {
        let parents = Dictionary(uniqueKeysWithValues: events.map { ($0.id, $0.causalParent) })
        return events.map { event in
            var seen = Set([event.id])
            var parent = event.causalParent
            while let id = parent, seen.insert(id).inserted { parent = parents[id] ?? nil }
            return seen.count
        }.max() ?? 0
    }

    private static func rootImpacts(_ events: [SimulationEvent]) -> [RootImpactSummary] {
        let parents = Dictionary(uniqueKeysWithValues: events.map { ($0.id, $0.causalParent) })
        let arrivals = events.filter { $0.kind == "input.descending-root-arrival" }
        var counts = Dictionary(uniqueKeysWithValues: arrivals.map { ($0.id, 0) })
        for event in events {
            var parent = event.causalParent
            var seen = Set<String>()
            while let id = parent, seen.insert(id).inserted {
                if counts[id] != nil { counts[id, default: 0] += 1; break }
                parent = parents[id] ?? nil
            }
        }
        return arrivals.map {
            RootImpactSummary(
                id: $0.id, root: $0.data["address"]?.string ?? "unknown",
                arrivalNS: $0.timeNS, consequences: counts[$0.id] ?? 0
            )
        }
    }

    private static func edgeLoads(_ events: [SimulationEvent]) -> [EdgeLoadSummary] {
        var loads: [String: (Int, Int, UInt64, UInt64, UInt64)] = [:]
        for event in events where event.kind.hasSuffix(".due") || event.kind.hasSuffix("-due") {
            guard let from = event.data["from"]?.int, let to = event.data["to"]?.int else { continue }
            let key = "\(from)-\(to)"
            let old = loads[key, default: (from, to, 0, 0, 0)]
            loads[key] = (
                from, to, old.2 + 1,
                old.3 + (event.data["transport_bytes"]?.uint64 ?? event.data["frame_bytes"]?.uint64 ?? 0),
                max(old.4, event.data["queue_occupancy_bytes"]?.uint64 ?? 0)
            )
        }
        return loads.values.map {
            EdgeLoadSummary(from: $0.0, to: $0.1, frames: $0.2, bytes: $0.3, peakQueueBytes: $0.4)
        }.sorted { $0.bytes == $1.bytes ? $0.id < $1.id : $0.bytes > $1.bytes }
    }

    private static func causalFlames(_ events: [SimulationEvent]) -> [CausalFlameSummary] {
        let byID = Dictionary(uniqueKeysWithValues: events.map { ($0.id, $0) })
        let inputs = events.filter { $0.kind.hasPrefix("input.") }
        var counts: [String: [String: UInt64]] = [:]
        for event in events {
            var cursor: SimulationEvent? = event
            var seen = Set<String>()
            while let current = cursor, seen.insert(current.id).inserted {
                if current.kind.hasPrefix("input.") {
                    counts[current.id, default: [:]][plane(event.kind), default: 0] += 1
                    break
                }
                cursor = current.causalParent.flatMap { byID[$0] }
            }
        }
        return inputs.compactMap { input in
            let slices = (counts[input.id] ?? [:]).map {
                DiagnosticBucket(label: $0.key, count: $0.value)
            }.sorted { $0.label < $1.label }
            let total = slices.reduce(0) { $0 + $1.count }
            guard total > 0 else { return nil }
            return CausalFlameSummary(
                id: input.id, label: input.kind, eventCount: total, slices: slices
            )
        }.sorted {
            $0.eventCount == $1.eventCount ? $0.id < $1.id : $0.eventCount > $1.eventCount
        }
    }

    private static func plane(_ kind: String) -> String {
        if kind.hasPrefix("tree-") { return "tree" }
        if kind.hasPrefix("bloom.") { return "Bloom" }
        if kind.hasPrefix("data.") { return "payload" }
        if kind.hasPrefix("lookup.") { return "lookup" }
        if kind.hasPrefix("session.") { return "session" }
        return kind.hasPrefix("input.") ? "input" : "other"
    }
}
