import Foundation

struct SearchMetric: Identifiable, Equatable, Sendable {
    var id: String { name }
    let name: String
    let value: UInt64
}

struct SearchSummary: Equatable, Sendable {
    let evaluated: Int
    let bestCaseID: String
    let metrics: [SearchMetric]
    let shrinkChanges: [String]
    let minimizedNodes: UInt64?

    static func parse(search: [String: JSONValue], shrink: [String: JSONValue]) -> SearchSummary? {
        guard let checkpoint = search["checkpoint"]?.object else { return nil }
        guard let evaluatedCases = checkpoint["evaluated"]?.object else { return nil }
        guard let bestValues = search["best_case_ids"]?.array else { return nil }
        guard let best = bestValues.first?.string else { return nil }
        guard let candidate = evaluatedCases[best]?.object else { return nil }
        let rawMetrics = candidate["metrics"]?.object ?? [:]
        var metrics: [SearchMetric] = []
        for (name, rawValue) in rawMetrics {
            if let value = rawValue.uint64 { metrics.append(SearchMetric(name: name, value: value)) }
        }
        metrics.sort { left, right in
            if left.value == right.value { return left.name < right.name }
            return left.value > right.value
        }
        let changes = acceptedChanges(shrink)
        let nodes = minimizedNodeCount(shrink)
        return SearchSummary(
            evaluated: evaluatedCases.count, bestCaseID: best, metrics: metrics,
            shrinkChanges: changes, minimizedNodes: nodes
        )
    }

    private static func acceptedChanges(_ shrink: [String: JSONValue]) -> [String] {
        let steps = shrink["steps"]?.array ?? []
        return steps.compactMap { value in
            guard let step = value.object, step["predicate_held"]?.bool == true else { return nil }
            return step["change"]?.string
        }
    }

    private static func minimizedNodeCount(_ shrink: [String: JSONValue]) -> UInt64? {
        guard let reproduction = shrink["reproduction"]?.object else { return nil }
        guard let plan = reproduction["normalized_plan"]?.object else { return nil }
        guard let campaign = plan["campaign"]?.object else { return nil }
        guard let scale = campaign["scale"]?.object else { return nil }
        return scale["nodes"]?.uint64
    }
}
