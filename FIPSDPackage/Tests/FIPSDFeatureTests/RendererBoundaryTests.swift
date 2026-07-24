import Foundation
import Testing
@testable import FIPSDFeature

@Test func canvasSourcesCannotBypassRenderFrame() throws {
    let names = [
        "NetworkCanvas.swift",
        "NetworkCanvasOverlays.swift",
        "NetworkCanvasPrimitives.swift",
        "CohortCanvas.swift",
        "CohortArtifactCanvas.swift"
    ]
    let source = rendererRepositoryRoot()
        .appendingPathComponent("FIPSDPackage/Sources/FIPSDFeature")
    for name in names {
        let text = try String(
            contentsOf: source.appendingPathComponent(name),
            encoding: .utf8
        )
        #expect(!text.contains("SimulationState"))
        #expect(!text.contains("CohortArtifactState"))
        #expect(!text.contains("state."))
    }
}

@Test func everyVisualizationModeHasOneFrameOwnedPrimitiveSet() throws {
    var state = evidenceState()
    state.nodes[1]?.parent = 0
    state.transmissions["flight"] = Transmission(
        id: "flight",
        from: 0,
        to: 1,
        startNS: 0,
        endNS: 100,
        frameBytes: 10,
        copy: 0,
        plane: "data"
    )
    state.lastRekeyAtNS[0] = 10

    let root = RenderFrame(
        state: state,
        virtualTimeNS: 50,
        visualizationMode: .rootAdoption
    )
    #expect(root.nodes.count == 2)
    #expect(root.parentRelations.count == 1)
    #expect(root.pulses.count == 1)
    #expect(root.cohorts.isEmpty)

    let connectivity = RenderFrame(
        state: state,
        virtualTimeNS: 50,
        visualizationMode: .connectivity
    )
    #expect(connectivity.nodes.count == 2)
    #expect(connectivity.parentRelations.isEmpty)
    #expect(connectivity.cohorts.isEmpty)

    let anomalies = RenderFrame(
        state: state,
        virtualTimeNS: 50,
        visualizationMode: .anomalies,
        anomalyNodeIDs: [0]
    )
    #expect(anomalies.nodes.map(\.state.id) == [0])
    #expect(anomalies.physicalLinks.isEmpty)
    #expect(anomalies.transmissions.isEmpty)
    #expect(anomalies.reconciliation.intentionallyOmitted > 0)

    let cohorts = RenderFrame(
        state: state,
        virtualTimeNS: 50,
        visualizationMode: .cohorts
    )
    #expect(cohorts.nodes.isEmpty)
    #expect(!cohorts.cohorts.isEmpty)
    #expect(cohorts.artifactCohorts.isEmpty)
}

@Test func pulsesArtifactCohortsAndSelectionAreEvidenceMapped() throws {
    var state = evidenceState()
    state.lastParentSwitchAtNS[1] = 100
    let cohortState = CohortArtifactState(
        representedNodes: 1_000_000_000,
        cohorts: [
            CohortRecord(
                id: "cohort-a",
                population: 1_000_000_000,
                depthStart: 0,
                depthEnd: 4,
                region: "eu",
                transport: "wifi",
                resource: "standard",
                protocolState: "active"
            )
        ],
        fidelity: "analytical cohort fixture"
    )
    let individual = RenderFrame(
        state: state,
        virtualTimeNS: 150,
        selectedNodeID: 1
    )
    let pulseEvidence = RenderFrameEvidence(
        frameIndex: 0,
        frame: individual,
        previous: nil
    )
    #expect(individual.selectedNodeID == 1)
    #expect(pulseEvidence.presentation.selectedNodeID == 1)
    #expect(
        pulseEvidence.primitives.pulses.first?.source
            == "state.lastParentSwitchAtNS[1]"
    )

    let cohort = RenderFrame(
        state: state,
        virtualTimeNS: 150,
        visualizationMode: .cohorts,
        cohortState: cohortState,
        displayBatch: .viewChange(at: 150)
    )
    let cohortEvidence = RenderFrameEvidence(
        frameIndex: 1,
        frame: cohort,
        previous: individual
    )
    #expect(cohortEvidence.violations.isEmpty)
    #expect(cohortEvidence.presentation.visualizationMode == .cohorts)
    #expect(cohortEvidence.presentation.anomalyNodeIDs.isEmpty)
    #expect(cohortEvidence.primitives.artifactCohorts.count == 1)
    #expect(cohortEvidence.primitives.nodes.isEmpty)
}

private func rendererRepositoryRoot() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
}
