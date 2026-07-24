import Foundation

struct VariantMetricDelta: Identifiable, Equatable, Sendable {
    var id: String { name }
    let name: String
    let unit: String
    let baseline: Int64
    let candidate: Int64
    var delta: Int64 { candidate - baseline }
}

struct VariantComparison: Equatable, Sendable {
    let baselineLabel: String
    let candidateLabel: String
    let compatible: Bool
    let compatibility: String
    let deltas: [VariantMetricDelta]
    let firstDivergence: String?
    let baseline: ArtifactAnalysis
    let candidate: ArtifactAnalysis

    static func compare(
        baseline: VariantRunEvidence,
        candidate: VariantRunEvidence,
        baselineDebounceMS: Double,
        candidateDebounceMS: Double
    ) -> VariantComparison {
        let compatible = baseline.analysis.representedNodes == candidate.analysis.representedNodes
        let left = values(baseline.analysis)
        let right = values(candidate.analysis)
        let units = (baseline.analysis.metrics + candidate.analysis.metrics).reduce(into: [String: String]()) {
            $0[$1.name] = $1.unit
        }
        let deltas = Set(left.keys).union(right.keys).compactMap { name -> VariantMetricDelta? in
            guard let a = left[name], let b = right[name] else { return nil }
            return VariantMetricDelta(
                name: name, unit: units[name] ?? unit(for: name), baseline: a, candidate: b
            )
        }.sorted { abs($0.delta) == abs($1.delta) ? $0.name < $1.name : abs($0.delta) > abs($1.delta) }
        return VariantComparison(
            baselineLabel: "Baseline \(format(baselineDebounceMS)) ms debounce",
            candidateLabel: "Candidate \(format(candidateDebounceMS)) ms debounce",
            compatible: compatible,
            compatibility: compatible
                ? "Same generated topology, seed, traffic, and represented population."
                : "Represented populations differ; deltas are informational only.",
            deltas: deltas,
            firstDivergence: firstDivergence(baseline.events, candidate.events),
            baseline: baseline.analysis,
            candidate: candidate.analysis
        )
    }

    private static func values(_ analysis: ArtifactAnalysis) -> [String: Int64] {
        var result = Dictionary(uniqueKeysWithValues: analysis.metrics.compactMap { metric in
            Int64(metric.value).map { (metric.name, $0) }
        })
        result["recorded-events"] = Int64(analysis.eventCount)
        result["causal-ledger-entries"] = Int64(analysis.ledgerEntries)
        result["longest-causal-chain"] = Int64(analysis.longestCausalChain)
        result["observed-link-bytes"] = Int64(clamping: analysis.topEdges.reduce(0) { $0 + $1.bytes })
        return result
    }

    private static func firstDivergence(
        _ left: [SimulationEvent], _ right: [SimulationEvent]
    ) -> String? {
        for (index, pair) in zip(left, right).enumerated() where pair.0 != pair.1 {
            return "event \(index): \(pair.0.kind) ↔ \(pair.1.kind)"
        }
        return left.count == right.count ? nil : "event \(min(left.count, right.count)): trace length"
    }

    private static func unit(for name: String) -> String {
        name.contains("bytes") ? "bytes" : "count"
    }

    private static func format(_ value: Double) -> String {
        value.formatted(.number.precision(.fractionLength(0...3)))
    }
}

struct VariantRunEvidence: Sendable {
    let analysis: ArtifactAnalysis
    let events: [SimulationEvent]
}
