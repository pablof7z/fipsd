import CoreGraphics
import Testing
@testable import FIPSDFeature

@Test func renderFrameUsesStableWorldCoordinates() throws {
    var state = makeRenderState(nodeIDs: [0, 1, 2])
    let before = RenderFrame(state: state, virtualTimeNS: 0)
    let original = try #require(before.nodes.first { $0.state.id == 1 }?.worldPoint)

    state.nodes[99] = renderNode(99)
    let afterJoin = RenderFrame(state: state, virtualTimeNS: 0)
    let afterJoinPoint = try #require(
        afterJoin.nodes.first { $0.state.id == 1 }?.worldPoint
    )
    let filtered = RenderFrame(
        state: state,
        virtualTimeNS: 0,
        visibleNodeIDs: [1, 99]
    )
    let filteredPoint = try #require(filtered.nodes.first { $0.state.id == 1 }?.worldPoint)

    #expect(original == afterJoinPoint)
    #expect(original == filteredPoint)
}

@Test func renderFrameIsIndependentOfDictionaryInsertionOrder() {
    var ascending = SimulationState()
    var descending = SimulationState()
    for id in [0, 1, 2] { ascending.nodes[id] = renderNode(id) }
    for id in [2, 1, 0] { descending.nodes[id] = renderNode(id) }
    let edges = [
        EdgeState(id: 3, from: 1, to: 2),
        EdgeState(id: 2, from: 0, to: 1)
    ]
    for edge in edges { ascending.edges[edge.id] = edge }
    for edge in edges.reversed() { descending.edges[edge.id] = edge }
    let flights = [
        Transmission(
            id: "b", from: 1, to: 2, startNS: 0, endNS: 100,
            frameBytes: 40, copy: 0, plane: "data"
        ),
        Transmission(
            id: "a", from: 0, to: 1, startNS: 0, endNS: 200,
            frameBytes: 20, copy: 0, plane: "control"
        )
    ]
    for flight in flights { ascending.transmissions[flight.id] = flight }
    for flight in flights.reversed() { descending.transmissions[flight.id] = flight }

    #expect(
        RenderFrame(state: ascending, virtualTimeNS: 50)
            == RenderFrame(state: descending, virtualTimeNS: 50)
    )
}

@Test func visiblePrimitivesReconcileExactlyToSimulationState() {
    var state = makeRenderState(nodeIDs: [0, 1, 2])
    state.nodes[1]?.parent = 0
    state.nodes[2]?.parent = 1
    state.edges[0] = EdgeState(id: 0, from: 0, to: 1)
    state.edges[1] = EdgeState(id: 1, from: 1, to: 2)
    state.transmissions["flight"] = Transmission(
        id: "flight", from: 0, to: 1, startNS: 100, endNS: 300,
        frameBytes: 128, copy: 0, plane: "bloom"
    )
    state.applicationTransfers["download"] = ApplicationTransferState(
        id: "download",
        source: 0,
        destination: 2,
        path: [0, 1, 2],
        totalBytes: 1_000,
        offeredBytes: 1_000,
        deliveredBytes: 100,
        startedAtNS: 100
    )

    let frame = RenderFrame(state: state, virtualTimeNS: 200)

    #expect(frame.reconciliation.isExact)
    #expect(frame.reconciliation.visibleNodes == 3)
    #expect(frame.reconciliation.visiblePhysicalLinks == 2)
    #expect(frame.reconciliation.visibleParentRelations == 2)
    #expect(frame.reconciliation.visibleRoutes == 1)
    #expect(frame.reconciliation.visibleTransmissions == 1)
    #expect(frame.reconciliation.intentionallyOmitted == 0)
    #expect(frame.physicalLinks.allSatisfy { $0.kind == .physicalLink })
    #expect(frame.parentRelations.allSatisfy { $0.kind == .parentRelation })
    #expect(frame.routes.allSatisfy { $0.kind == .route })
    #expect(frame.transmissions.allSatisfy { $0.kind == .transmission })
    #expect(frame.transmissions.first?.progress == 0.5)
}

@Test func malformedRenderSourcesAreDeclaredRatherThanSilentlyDrawn() {
    var state = makeRenderState(nodeIDs: [0])
    state.edges[3] = EdgeState(id: 3, from: 0, to: 404)
    state.transmissions["missing"] = Transmission(
        id: "missing", from: 0, to: 404, startNS: 0, endNS: 1,
        frameBytes: 1, copy: 0, plane: "data"
    )

    let frame = RenderFrame(state: state, virtualTimeNS: 0)

    #expect(!frame.reconciliation.isExact)
    #expect(frame.reconciliation.violations.count == 2)
    #expect(frame.physicalLinks.isEmpty)
    #expect(frame.transmissions.isEmpty)
}

@Test func missingParentEndpointIsAReconciliationViolation() {
    var state = makeRenderState(nodeIDs: [0])
    state.nodes[0]?.parent = 404

    let frame = RenderFrame(state: state, virtualTimeNS: 0)

    #expect(!frame.reconciliation.isExact)
    #expect(frame.reconciliation.violations == [
        "1 parent relations lack endpoints"
    ])
}

@Test func cohortPositionsDoNotMoveWhenRootRankingsChange() throws {
    var state = makeRenderState(nodeIDs: [0, 1, 2, 3, 4])
    state.nodes[0]?.root = 10
    state.nodes[1]?.root = 10
    state.nodes[2]?.root = 20
    state.nodes[3]?.root = 20
    state.nodes[4]?.root = 20
    let size = CGSize(width: 900, height: 700)
    let beforeFrame = RenderFrame(state: state, virtualTimeNS: 0)
    let before = try #require(CohortLayout(frame: beforeFrame, size: size).position(of: 0))
    let beforeWorld = try #require(
        CohortLayout(frame: beforeFrame, size: size).worldPoint(of: 0)
    )

    for id in 5..<20 {
        state.nodes[id] = renderNode(id, root: 20)
    }
    let afterFrame = RenderFrame(state: state, virtualTimeNS: 0)
    let after = try #require(CohortLayout(frame: afterFrame, size: size).position(of: 0))
    let resizedWorld = try #require(
        CohortLayout(
            frame: afterFrame,
            size: CGSize(width: 1_200, height: 500)
        ).worldPoint(of: 0)
    )

    #expect(before == after)
    #expect(beforeWorld == resizedWorld)
}

@Test func cohortFlightAggregationIsDeterministicAndUsesEveryFlight() {
    var state = makeRenderState(nodeIDs: [0, 1])
    state.transmissions["late"] = Transmission(
        id: "late", from: 0, to: 1, startNS: 0, endNS: 200,
        frameBytes: 1, copy: 0, plane: "data"
    )
    state.transmissions["early"] = Transmission(
        id: "early", from: 0, to: 1, startNS: 0, endNS: 100,
        frameBytes: 1, copy: 0, plane: "data"
    )
    let frame = RenderFrame(state: state, virtualTimeNS: 50)
    let layout = CohortLayout(frame: frame, size: CGSize(width: 800, height: 600))
    let aggregates = layout.flightAggregates

    #expect(aggregates.count == 1)
    #expect(aggregates.first?.count == 2)
    #expect(aggregates.first?.meanProgress == 0.375)

    var reordered = state
    reordered.transmissions = [:]
    reordered.transmissions["early"] = state.transmissions["early"]
    reordered.transmissions["late"] = state.transmissions["late"]
    let reorderedFrame = RenderFrame(state: reordered, virtualTimeNS: 50)
    let reorderedLayout = CohortLayout(
        frame: reorderedFrame,
        size: CGSize(width: 800, height: 600)
    )
    #expect(aggregates == reorderedLayout.flightAggregates)
}

@Test func compressedDisplayBatchIsExplicit() throws {
    let events = try (0..<3).map { index in
        try #require(renderEvent(id: index))
    }
    let batch = DisplayProjectionBatch(
        events: events,
        fromNS: 0,
        throughNS: 16,
        mode: .exactSummary,
        compressionReason: .playbackWindowDensity
    )

    #expect(batch.count == 3)
    #expect(batch.isCompressed)
    #expect(batch.label.contains("3 ordered events exactly summarized"))
}

private func makeRenderState(nodeIDs: [Int]) -> SimulationState {
    var state = SimulationState()
    for id in nodeIDs { state.nodes[id] = renderNode(id) }
    return state
}

private func renderNode(_ id: Int, root: Int? = nil) -> NodeState {
    NodeState(
        id: id,
        address: String(format: "%032x", id),
        active: true,
        root: root ?? id,
        parent: nil,
        sequence: 1
    )
}

private func renderEvent(id: Int) -> SimulationEvent? {
    SimulationEvent(.object([
        "event_id": .string("event-\(id)"),
        "virtual_time_ns": .integer(Int64(id)),
        "ordinal": .integer(Int64(id)),
        "kind": .string("test.event"),
        "causal_parent": .null,
        "data": .object([:])
    ]))
}
