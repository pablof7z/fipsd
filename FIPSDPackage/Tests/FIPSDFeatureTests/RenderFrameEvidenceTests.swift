import Foundation
import Testing
@testable import FIPSDFeature

@Test func frameEvidenceMapsPrimitivesAndChangesToSources() throws {
    var state = evidenceState()
    let initialEvent = try #require(evidenceEvent(
        id: "initial",
        ordinal: 0,
        kind: "input.initial-topology"
    ))
    let initialBatch = DisplayProjectionBatch(
        events: [initialEvent],
        fromNS: 0,
        throughNS: 0,
        mode: .orderedEvent
    )
    let before = RenderFrame(
        state: state,
        virtualTimeNS: 0,
        displayBatch: initialBatch,
        sourceFidelity: "individual · semantic-exact · executable-codec"
    )
    let initial = RenderFrameEvidence(
        frameIndex: 0,
        frame: before,
        previous: nil
    )

    state.nodes[1]?.root = 0
    state.nodes[1]?.parent = 0
    let delivery = try #require(evidenceEvent(
        id: "delivery",
        ordinal: 1,
        kind: "tree-announce.delivered",
        causalParent: "send"
    ))
    let batch = DisplayProjectionBatch(
        events: [delivery],
        fromNS: 0,
        throughNS: 10,
        mode: .orderedEvent
    )
    let after = RenderFrame(
        state: state,
        virtualTimeNS: 10,
        displayBatch: batch,
        sourceFidelity: "individual · semantic-exact · executable-codec"
    )
    let evidence = RenderFrameEvidence(
        frameIndex: 1,
        frame: after,
        previous: before
    )

    #expect(initial.violations.isEmpty)
    #expect(evidence.violations.isEmpty)
    #expect(evidence.delta.changedNodeIDs == [1])
    #expect(evidence.delta.changedParentNodeIDs == [1])
    #expect(evidence.delta.layoutOnlyNodeIDs.isEmpty)
    #expect(evidence.primitives.nodes.first?.source == "state.nodes[0]")
    #expect(evidence.primitives.physicalLinks.first?.source == "state.edges[0]")
    #expect(
        evidence.primitives.parentRelations.first?.source
            == "state.nodes[1].parent"
    )
    #expect(
        evidence.primitives.routes.first?.source
            == "state.applicationTransfers[download]"
    )
    #expect(evidence.presentation.eventIDs == ["delivery"])
    #expect(evidence.presentation.initiatingEventIDs == ["delivery"])
}

@Test func nativeFrameWriterEmitsOrderedDeterministicJSONL() async throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("render-writer-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let writer = try RenderFrameLogWriter(directory: directory)
    let event = try #require(evidenceEvent(
        id: "event-0",
        ordinal: 0,
        kind: "input.initial-topology"
    ))
    let batch = DisplayProjectionBatch(
        events: [event],
        fromNS: 0,
        throughNS: 0,
        mode: .orderedEvent
    )
    let frame = RenderFrame(
        state: evidenceState(),
        virtualTimeNS: 0,
        displayBatch: batch,
        sourceFidelity: "semantic-exact"
    )
    for index in 0..<2 {
        try writer.append(RenderFrameEvidence(
            frameIndex: index,
            frame: frame,
            previous: index == 0 ? nil : frame
        ))
    }
    try await writer.flush()

    let text = try String(contentsOf: writer.outputURL, encoding: .utf8)
    let lines = text.split(separator: "\n")
    #expect(lines.count == 2)
    #expect(lines[0].hasPrefix("{\"delta\":"))
    let first = try #require(
        JSONSerialization.jsonObject(with: Data(lines[0].utf8))
            as? [String: Any]
    )
    let second = try #require(
        JSONSerialization.jsonObject(with: Data(lines[1].utf8))
            as? [String: Any]
    )
    #expect(first["frame_index"] as? Int == 0)
    #expect(second["frame_index"] as? Int == 1)
    #expect(
        first["schema"] as? String
            == "experiments.fips.network/render-frame/v1alpha1"
    )
}

@Test func minimalRendererEvidenceMatchesCommittedFixture() throws {
    let frame = RenderFrame(
        state: SimulationState(),
        virtualTimeNS: 0,
        sourceFidelity: "fixture"
    )
    let evidence = RenderFrameEvidence(
        frameIndex: 0,
        frame: frame,
        previous: nil
    )
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
    let actual = try JSONDecoder().decode(
        JSONValue.self,
        from: encoder.encode(evidence)
    )
    let expected = try JSONDecoder().decode(
        JSONValue.self,
        from: Data(contentsOf: repositoryRoot()
            .appendingPathComponent("fixtures/renderer/render-frame-minimal.json"))
    )

    #expect(actual == expected)
}

@MainActor
@Test func campaignFidelityIsAvailableBeforeFreshPlayback() throws {
    let campaign = try JSONSerialization.data(withJSONObject: [
        "fidelity": [
            "scale": "individual",
            "protocol": "semantic-exact",
            "wire": "executable-codec"
        ]
    ])
    let model = WorkbenchModel()

    #expect(
        model.declaredRendererFidelity(from: campaign)
            == "declared campaign · individual · semantic-exact · executable-codec"
    )
}

@Test func committedRendererAuditArtifactsReplayWithoutUnattributedChanges() throws {
    let paths = [
        "root-ratchet-12/evidence/artifact.json",
        "observed-1000/evidence/artifact.json"
    ]
    var subFrameTransmissionEvents = 0
    for path in paths {
        let result = try replayAuditArtifact(path)
        #expect(result.replayedEventIDs == result.sourceEventIDs)
        #expect(result.compressedFrames > 0)
        #expect(result.violations.isEmpty)
        subFrameTransmissionEvents += result.subFrameTransmissionEvents
    }
    #expect(subFrameTransmissionEvents > 0)
}

private struct AuditReplayResult {
    let sourceEventIDs: [String]
    let replayedEventIDs: [String]
    let compressedFrames: Int
    let subFrameTransmissionEvents: Int
    let violations: [String]
}

private func replayAuditArtifact(_ relativePath: String) throws -> AuditReplayResult {
    let url = repositoryRoot()
        .appendingPathComponent("reports/renderer-audit-2026-07-24")
        .appendingPathComponent(relativePath)
    let root = try JSONDecoder().decode(
        JSONValue.self,
        from: Data(contentsOf: url)
    )
    let values = try #require(root.object?["event_trace"]?.array)
    let events = try values.map { try #require(SimulationEvent($0)) }
    let scheduler = EventAwarePlaybackScheduler()
    var state = SimulationState()
    var previous: RenderFrame?
    var cursor = 0
    var timeNS: UInt64 = 0
    var replayed: [String] = []
    var compressed = 0
    var shortFlights = 0
    var violations: [String] = []
    var frameIndex = 0
    while cursor < events.count {
        let update = scheduler.nextUpdate(
            events: events,
            cursor: cursor,
            virtualTimeNS: timeNS,
            wallDeltaNS: 16_000_000,
            speed: 1
        )
        let applied = Array(events[update.eventRange])
        for event in applied {
            state.apply(event)
            if event.data["deliveries"]?.array?.contains(where: {
                guard let end = $0.object?["deliver_at_ns"]?.uint64 else { return false }
                return end.saturatingSubtract(event.timeNS) < 16_000_000
            }) == true {
                shortFlights += 1
            }
        }
        cursor = update.eventRange.upperBound
        timeNS = max(timeNS, update.throughNS)
        state.expireTransmissions(at: timeNS)
        let batch = DisplayProjectionBatch(
            events: applied,
            fromNS: previous?.virtualTimeNS ?? 0,
            throughNS: timeNS,
            mode: update.mode,
            compressionReason: update.compressionReason
        )
        let frame = RenderFrame(
            state: state,
            virtualTimeNS: timeNS,
            displayBatch: batch,
            sourceFidelity: "committed audit artifact"
        )
        let evidence = RenderFrameEvidence(
            frameIndex: frameIndex,
            frame: frame,
            previous: previous
        )
        replayed.append(contentsOf: batch.eventIDs)
        if batch.isCompressed { compressed += 1 }
        violations.append(contentsOf: evidence.violations)
        previous = frame
        frameIndex += 1
    }
    return AuditReplayResult(
        sourceEventIDs: events.map(\.id),
        replayedEventIDs: replayed,
        compressedFrames: compressed,
        subFrameTransmissionEvents: shortFlights,
        violations: violations
    )
}

private func repositoryRoot() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
}
