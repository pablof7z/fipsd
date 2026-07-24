import Foundation

struct ScaleEstimate: Equatable, Sendable {
    let value: String
    let lower: String
    let upper: String
    let unit: String
    let method: String
    let uncertainty: String

    var numericValue: Double { Double(value) ?? 0 }

    static func parse(_ value: JSONValue?) -> ScaleEstimate? {
        guard let object = value?.object,
              let central = object["value"]?.string,
              let lower = object["lower"]?.string,
              let upper = object["upper"]?.string else { return nil }
        return ScaleEstimate(
            value: central, lower: lower, upper: upper,
            unit: object["unit"]?.string ?? "",
            method: object["method"]?.string ?? "unknown",
            uncertainty: object["uncertainty"]?.string ?? "unknown"
        )
    }
}

struct ScaleScenario: Identifiable, Equatable, Sendable {
    var id: String { "\(topology)-\(cadenceNS)-\(variant)" }
    let topology: String
    let cadenceNS: UInt64
    let variant: String
    let controlBytes: ScaleEstimate
    let peakQueueBytes: ScaleEstimate
    let bloomFPR: ScaleEstimate
    let maximumDepth: ScaleEstimate

    var shortVariant: String {
        if variant.contains("dampening") { return "dampening" }
        if variant.contains("bloom-delta") { return "Bloom delta" }
        return "baseline"
    }
}

struct ScaleSensitivity: Equatable, Sendable {
    let representedNodes: UInt64
    let claim: String
    let warning: String
    let scenarios: [ScaleScenario]
    let minimumControlBytes: String
    let maximumControlBytes: String
    let exactSampleNodes: UInt64
    let maximumCohorts: UInt64

    var controlSpan: Double {
        let minimum = Double(minimumControlBytes) ?? 0
        let maximum = Double(maximumControlBytes) ?? 0
        return minimum > 0 ? maximum / minimum : 0
    }

    static func parse(_ root: [String: JSONValue]) -> ScaleSensitivity? {
        guard root["kind"]?.string == "honest-billion-node-root-ratchet/v1alpha1",
              let represented = root["represented_nodes"]?.uint64,
              let values = root["scenarios"]?.array else { return nil }
        let scenarios = values.compactMap(parseScenario)
        guard scenarios.count == values.count, !scenarios.isEmpty else { return nil }
        let budget = root["resource_budget"]?.object
        return ScaleSensitivity(
            representedNodes: represented,
            claim: root["representation_claim"]?.string ?? "unknown representation",
            warning: root["headline_warning"]?.string ?? "",
            scenarios: scenarios,
            minimumControlBytes: root["minimum_control_bytes"]?.string ?? "0",
            maximumControlBytes: root["maximum_control_bytes"]?.string ?? "0",
            exactSampleNodes: budget?["maximum_exact_nodes"]?.uint64 ?? 0,
            maximumCohorts: budget?["maximum_allocated_cohorts"]?.uint64 ?? 0
        )
    }

    private static func parseScenario(_ value: JSONValue) -> ScaleScenario? {
        guard let object = value.object,
              let topology = object["topology"]?.string,
              let cadence = object["cadence_ns"]?.uint64,
              let variant = object["variant"]?.string,
              let control = ScaleEstimate.parse(object["control_bytes"]),
              let queue = ScaleEstimate.parse(object["peak_queue_bytes"]),
              let bloom = ScaleEstimate.parse(object["bloom_fpr_ppb"]),
              let depth = ScaleEstimate.parse(object["maximum_depth"]) else { return nil }
        return ScaleScenario(
            topology: topology, cadenceNS: cadence, variant: variant,
            controlBytes: control, peakQueueBytes: queue,
            bloomFPR: bloom, maximumDepth: depth
        )
    }
}
