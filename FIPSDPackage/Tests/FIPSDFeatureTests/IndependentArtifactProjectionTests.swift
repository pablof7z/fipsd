import Foundation
import Testing
@testable import FIPSDFeature

private struct RawArtifactCohort: Equatable {
    let id: String
    let population: UInt64
    let depthStart: UInt64
    let depthEnd: UInt64
    let region: String
    let transport: String
    let resource: String
    let protocolState: String
}

@Test func billionScaleArtifactMarksMatchIndependentJSONProjection() throws {
    let url = artifactProjectionRepositoryRoot()
        .appendingPathComponent("fixtures/m4/billion-cohort-artifact.json")
    let rootValue = try JSONDecoder().decode(
        JSONValue.self,
        from: Data(contentsOf: url)
    )
    let root = try #require(rootValue.object)
    let fidelity = try #require(root["manifest"]?.object?["fidelity"]?.object)
    let representedNodes = try #require(fidelity["represented_nodes"]?.uint64)
    let rawCohorts = try #require(
        root["samples"]?.array?.first?.object?["cohorts"]?.array
    )
    let expected = rawCohorts.compactMap(rawArtifactCohort).sorted {
        $0.id < $1.id
    }
    let parsed = try #require(CohortArtifactState.parse(root))
    let frame = RenderFrame(
        state: SimulationState(),
        virtualTimeNS: 0,
        visualizationMode: .cohorts,
        cohortState: parsed,
        displayBatch: .viewChange(at: 0),
        sourceFidelity: "committed billion-node artifact"
    )
    let actual = frame.artifactCohorts.map {
        RawArtifactCohort(
            id: $0.id,
            population: $0.population,
            depthStart: $0.depthStart,
            depthEnd: $0.depthEnd,
            region: $0.region,
            transport: $0.transport,
            resource: $0.resource,
            protocolState: $0.protocolState
        )
    }
    let evidence = RenderFrameEvidence(
        frameIndex: 0,
        frame: frame,
        previous: nil
    )

    #expect(frame.representedNodes == representedNodes)
    #expect(actual == expected)
    #expect(frame.nodes.isEmpty)
    #expect(frame.cohorts.isEmpty)
    #expect(evidence.violations.isEmpty)
    #expect(evidence.primitives.artifactCohorts.count == expected.count)
    #expect(
        evidence.primitives.artifactCohorts.map(\.source)
            == expected.map {
                "artifact.samples[0].cohorts[id=\($0.id)]"
            }
    )
}

private func rawArtifactCohort(_ value: JSONValue) -> RawArtifactCohort? {
    guard let object = value.object,
          let key = object["key"]?.object,
          let id = object["id"]?.string,
          let population = object["population"]?.uint64 else { return nil }
    return RawArtifactCohort(
        id: id,
        population: population,
        depthStart: key["depth_start"]?.uint64 ?? 0,
        depthEnd: key["depth_end"]?.uint64 ?? 0,
        region: key["region"]?.string ?? "unknown",
        transport: key["transport"]?.string ?? "unknown",
        resource: key["resource"]?.string ?? "unknown",
        protocolState: key["protocol_state"]?.string ?? "unknown"
    )
}

private func artifactProjectionRepositoryRoot() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
}
