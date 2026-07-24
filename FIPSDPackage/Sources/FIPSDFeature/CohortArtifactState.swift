import Foundation

struct CohortRecord: Identifiable, Equatable, Sendable {
    let id: String
    let population: UInt64
    let depthStart: UInt64
    let depthEnd: UInt64
    let region: String
    let transport: String
    let resource: String
    let protocolState: String
}

struct CohortArtifactState: Equatable, Sendable {
    let representedNodes: UInt64
    let cohorts: [CohortRecord]
    let fidelity: String

    static func parse(_ root: [String: JSONValue]) -> CohortArtifactState? {
        guard let fidelityObject = root["manifest"]?.object?["fidelity"]?.object,
              fidelityObject["scale"]?.string == "cohort",
              let represented = fidelityObject["represented_nodes"]?.uint64,
              let sample = root["samples"]?.array?.first?.object,
              let values = sample["cohorts"]?.array else { return nil }
        let cohorts = values.compactMap { value -> CohortRecord? in
            guard let object = value.object, let key = object["key"]?.object,
                  let id = object["id"]?.string,
                  let population = object["population"]?.uint64 else { return nil }
            return CohortRecord(
                id: id, population: population,
                depthStart: key["depth_start"]?.uint64 ?? 0,
                depthEnd: key["depth_end"]?.uint64 ?? 0,
                region: key["region"]?.string ?? "unknown",
                transport: key["transport"]?.string ?? "unknown",
                resource: key["resource"]?.string ?? "unknown",
                protocolState: key["protocol_state"]?.string ?? "unknown"
            )
        }
        guard !cohorts.isEmpty else { return nil }
        let approximations = fidelityObject["approximations"]?.array?.count ?? 0
        return CohortArtifactState(
            representedNodes: represented, cohorts: cohorts,
            fidelity: "analytical cohorts · \(approximations) declared approximation label(s)"
        )
    }
}
