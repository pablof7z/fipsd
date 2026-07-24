import CoreGraphics
import Testing
@testable import FIPSDFeature

/// Reproduces the "make a 5 node network" case that rendered as an unreadable
/// blob in the top-left corner: a shallow network where every node is depth-band
/// 0, so the synthetic cohort world points cluster tightly. Fit-to-content must
/// spread them across the canvas with no overlapping bubbles.
private func shallowFiveNodeState() -> SimulationState {
    var state = SimulationState()
    let plan: [(id: Int, root: Int, transport: String)] = [
        (0, 2, "wifi"),
        (1, 2, "ble"),
        (2, 2, "tor"),
        (3, 3, "wifi"),
        (4, 4, "ethernet")
    ]
    for entry in plan {
        state.nodes[entry.id] = NodeState(
            id: entry.id,
            address: String(format: "%032x", entry.id),
            active: true,
            root: entry.root,
            parent: nil,
            sequence: 1,
            transportProfile: entry.transport,
            transportType: entry.transport
        )
    }
    return state
}

@Test func sparseCohortsFillCanvasWithoutOverlap() {
    let frame = RenderFrame(state: shallowFiveNodeState(), virtualTimeNS: 0, visualizationMode: .cohorts)
    let size = CGSize(width: 1_440, height: 900)
    let layout = CohortLayout(frame: frame, size: size)
    let points = frame.cohorts.compactMap { layout.position(of: $0.nodeIDs.min() ?? -1) }

    #expect(points.count == frame.cohorts.count)

    // Every mark lands inside the canvas.
    for point in points {
        #expect(point.x >= 0 && point.x <= size.width)
        #expect(point.y >= 0 && point.y <= size.height)
    }

    // Content spreads across a meaningful fraction of the canvas rather than
    // collapsing into a corner sliver. The synthetic layout is deterministically
    // hash-positioned, so a given topology may be thin on one axis; fit-to-content
    // guarantees the marks fill a meaningful fraction of at least one axis.
    let spanX = (points.map(\.x).max() ?? 0) - (points.map(\.x).min() ?? 0)
    let spanY = (points.map(\.y).max() ?? 0) - (points.map(\.y).min() ?? 0)
    #expect(max(spanX / size.width, spanY / size.height) > 0.35)

    // No two cohort bubbles overlap.
    for outer in 0..<frame.cohorts.count {
        for inner in (outer + 1)..<frame.cohorts.count {
            let left = points[outer]
            let right = points[inner]
            let distance = hypot(left.x - right.x, left.y - right.y)
            let radii = (
                RenderMarkMetrics.cohortDiameter(nodeCount: frame.cohorts[outer].nodeIDs.count)
                    + RenderMarkMetrics.cohortDiameter(nodeCount: frame.cohorts[inner].nodeIDs.count)
            ) / 2
            #expect(distance >= radii)
        }
    }
}

@Test func layoutEvidenceReportsNoOverlapForSparseNetwork() {
    let frame = RenderFrame(state: shallowFiveNodeState(), virtualTimeNS: 0, visualizationMode: .cohorts)
    let evidence = RenderLayoutEvidence(frame)

    #expect(evidence.cohorts.count == 5)
    #expect(evidence.cohortsOverlap == false)
    #expect(evidence.minCohortSeparation > RenderMarkMetrics.cohortDiameter(nodeCount: 1))
    #expect(evidence.referenceCanvasWidth == 1_440)
    #expect(evidence.referenceCanvasHeight == 900)
}
