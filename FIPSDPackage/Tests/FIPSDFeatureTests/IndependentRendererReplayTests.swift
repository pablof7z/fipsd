import Foundation
import Testing
@testable import FIPSDFeature

@Test func independentOracleDoesNotImportRendererStateMachinery() throws {
    let directory = renderProofRepositoryRoot()
        .appendingPathComponent("FIPSDPackage/Tests/FIPSDFeatureTests")
    let files = [
        "IndependentRenderOracle.swift",
        "IndependentRenderOracleTraffic.swift",
        "IndependentRenderOracleManifest.swift"
    ]
    let forbidden = [
        "SimulationEvent",
        "SimulationState",
        "RenderFrame",
        "RenderSourceProjection",
        "CohortProjection"
    ]
    for file in files {
        let source = try String(
            contentsOf: directory.appendingPathComponent(file),
            encoding: .utf8
        )
        for name in forbidden {
            #expect(!source.contains(name))
        }
    }
}

@Test func committedAuditArtifactsMatchIndependentVisibleOracle() throws {
    let paths = [
        "root-ratchet-12/evidence/artifact.json",
        "observed-1000/evidence/artifact.json"
    ]
    var comparedFrames = 0
    for path in paths {
        comparedFrames += try compareAuditArtifactToOracle(path)
    }
    #expect(comparedFrames > 0)
}

private func compareAuditArtifactToOracle(_ path: String) throws -> Int {
    let url = renderProofRepositoryRoot()
        .appendingPathComponent("reports/renderer-audit-2026-07-24")
        .appendingPathComponent(path)
    let root = try JSONDecoder().decode(
        JSONValue.self,
        from: Data(contentsOf: url)
    )
    let rawEvents = try #require(root.object?["event_trace"]?.array)
    let events = try rawEvents.map { try #require(SimulationEvent($0)) }
    let anomalyNodes = IndependentRenderOracle.anomalyNodeIDs(rawEvents)
    let analyzedNodes = ArtifactAnalysis.parse(
        try #require(root.object)
    ).anomalyNodeIDs
    #expect(analyzedNodes == anomalyNodes)
    let scheduler = EventAwarePlaybackScheduler()
    var state = SimulationState()
    var oracle = IndependentRenderOracle()
    var previousFrames: [String: RenderFrame] = [:]
    var cursor = 0
    var timeNS: UInt64 = 0
    var compared = 0
    var frameIndex = 0

    while cursor < events.count {
        let update = scheduler.nextUpdate(
            events: events,
            cursor: cursor,
            virtualTimeNS: timeNS,
            wallDeltaNS: 16_000_000,
            speed: 1
        )
        let appliedEvents = Array(events[update.eventRange])
        let appliedRaw = Array(rawEvents[update.eventRange])
        for (event, raw) in zip(appliedEvents, appliedRaw) {
            state.apply(event)
            oracle.apply(raw)
        }
        cursor = update.eventRange.upperBound
        timeNS = max(timeNS, update.throughNS)
        state.expireTransmissions(at: timeNS)
        oracle.expire(at: timeNS)
        let batch = DisplayProjectionBatch(
            events: appliedEvents,
            fromNS: previousFrames.values.first?.virtualTimeNS ?? 0,
            throughNS: timeNS,
            mode: update.mode,
            compressionReason: update.compressionReason
        )
        for mode in VisualizationMode.allCases {
            let frame = RenderFrame(
                state: state,
                virtualTimeNS: timeNS,
                visualizationMode: mode,
                anomalyNodeIDs: analyzedNodes,
                displayBatch: batch,
                sourceFidelity: "committed audit artifact"
            )
            let expected = oracle.visibleManifest(
                mode: mode,
                virtualTimeNS: timeNS,
                anomalyNodeIDs: anomalyNodes
            )
            #expect(
                frame.anomalyNodeIDs
                    == (mode == .anomalies ? anomalyNodes.sorted() : [])
            )
            #expect(
                frame.semanticVisibleManifest == expected,
                "oracle mismatch in \(path), frame \(frameIndex), mode \(mode.rawValue)"
            )
            let evidence = RenderFrameEvidence(
                frameIndex: frameIndex,
                frame: frame,
                previous: previousFrames[mode.rawValue]
            )
            #expect(
                evidence.violations.isEmpty,
                "evidence violation in \(path), frame \(frameIndex), mode \(mode.rawValue)"
            )
            previousFrames[mode.rawValue] = frame
            compared += 1
        }
        frameIndex += 1
    }
    return compared
}

private func renderProofRepositoryRoot() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
}
