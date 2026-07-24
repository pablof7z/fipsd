import CoreGraphics
import Foundation

enum RenderPrimitiveKind: String, Equatable, Sendable {
    case physicalLink = "physical-link"
    case parentRelation = "parent-relation"
    case route = "application-route"
    case transmission = "in-flight-transmission"
}

struct RenderWorldPoint: Equatable, Sendable {
    let x: Double
    let y: Double

    func projected(in size: CGSize, margin: CGFloat = 54) -> CGPoint {
        let width = max(1, size.width - margin * 2)
        let height = max(1, size.height - margin * 2)
        return CGPoint(
            x: margin + width * CGFloat((x + 1) / 2),
            y: margin + height * CGFloat((y + 1) / 2)
        )
    }
}

struct RenderNode: Equatable, Sendable {
    let state: NodeState
    let worldPoint: RenderWorldPoint
}

struct RenderPhysicalLink: Equatable, Sendable {
    let kind = RenderPrimitiveKind.physicalLink
    let edge: EdgeState
}

struct RenderParentRelation: Equatable, Sendable {
    let kind = RenderPrimitiveKind.parentRelation
    let child: Int
    let parent: Int
}

struct RenderRoute: Equatable, Sendable {
    let kind = RenderPrimitiveKind.route
    let transfer: ApplicationTransferState

    var transferID: String { transfer.id }
    var nodeIDs: [Int] { transfer.path }
}

struct RenderTransmission: Equatable, Sendable {
    let kind = RenderPrimitiveKind.transmission
    let transmission: Transmission
    let progress: Double
}

struct RenderReconciliation: Equatable, Sendable {
    let sourceNodes: Int
    let visibleNodes: Int
    let sourcePhysicalLinks: Int
    let visiblePhysicalLinks: Int
    let sourceParentRelations: Int
    let visibleParentRelations: Int
    let sourceRoutes: Int
    let visibleRoutes: Int
    let sourceTransmissions: Int
    let visibleTransmissions: Int
    let sourcePulses: Int
    let visiblePulses: Int
    let sourceArtifactCohorts: Int
    let visibleArtifactCohorts: Int
    let intentionallyOmitted: Int
    let violations: [String]

    var isExact: Bool { violations.isEmpty }
}

struct RenderFrame: Equatable, Sendable {
    let virtualTimeNS: UInt64
    let visualizationMode: VisualizationMode
    let anomalyNodeIDs: [Int]
    let selectedNodeID: Int?
    let displayBatch: DisplayProjectionBatch
    let sourceFidelity: String
    let representedNodes: UInt64
    let sourceActiveNodes: Int
    let nodes: [RenderNode]
    let physicalLinks: [RenderPhysicalLink]
    let parentRelations: [RenderParentRelation]
    let routes: [RenderRoute]
    let transmissions: [RenderTransmission]
    let pulses: [RenderPulse]
    let cohorts: [RenderCohort]
    let cohortTransmissions: [CohortFlightAggregate]
    let artifactCohorts: [RenderArtifactCohort]
    let artifactCohortFidelity: String?
    let reconciliation: RenderReconciliation

    init(
        state: SimulationState,
        virtualTimeNS: UInt64,
        visualizationMode: VisualizationMode = .rootAdoption,
        anomalyNodeIDs: Set<Int> = [],
        cohortState: CohortArtifactState? = nil,
        selectedNodeID: Int? = nil,
        displayBatch: DisplayProjectionBatch = .empty,
        sourceFidelity: String = "unspecified renderer source fidelity"
    ) {
        self.virtualTimeNS = virtualTimeNS
        self.visualizationMode = visualizationMode
        self.displayBatch = displayBatch
        self.sourceFidelity = sourceFidelity
        let source = RenderSourceProjection(
            state: state,
            virtualTimeNS: virtualTimeNS,
            cohortState: cohortState
        )
        representedNodes = source.representedNodes
        sourceActiveNodes = source.sourceActiveNodes
        let individualMode = visualizationMode != .cohorts
        let anomalyIDs = visualizationMode == .anomalies
            ? anomalyNodeIDs.sorted()
            : []
        self.anomalyNodeIDs = anomalyIDs
        let included = visualizationMode == .anomalies
            ? Set(anomalyIDs)
            : Set(source.nodes.map(\.state.id))
        nodes = individualMode
            ? source.nodes.filter { included.contains($0.state.id) }
            : []
        self.selectedNodeID = selectedNodeID.flatMap { selected in
            individualMode && included.contains(selected) ? selected : nil
        }
        physicalLinks = individualMode ? source.physicalLinks.filter {
            included.contains($0.edge.from) && included.contains($0.edge.to)
        } : []
        parentRelations = visualizationMode == .rootAdoption
            ? source.parentRelations.filter {
                included.contains($0.child) && included.contains($0.parent)
            }
            : []
        routes = individualMode ? source.routes.filter {
            $0.nodeIDs.allSatisfy(included.contains)
        } : []
        transmissions = individualMode ? source.transmissions.filter {
            included.contains($0.transmission.from)
                && included.contains($0.transmission.to)
        } : []
        pulses = individualMode ? source.pulses.filter {
            included.contains($0.nodeID)
        } : []
        let usesArtifactCohorts = visualizationMode == .cohorts
            && !source.artifactCohorts.isEmpty
        cohorts = visualizationMode == .cohorts && !usesArtifactCohorts
            ? source.cohorts
            : []
        cohortTransmissions = visualizationMode == .cohorts
            && !usesArtifactCohorts
            ? source.cohortTransmissions
            : []
        artifactCohorts = usesArtifactCohorts ? source.artifactCohorts : []
        artifactCohortFidelity = usesArtifactCohorts
            ? source.artifactCohortFidelity
            : nil
        let sourceParents = state.nodes.values.filter { $0.parent != nil }.count
        let omitted = state.transmissions.count - transmissions.count
            + state.edges.count - physicalLinks.count
            + sourceParents - parentRelations.count
            + state.applicationTransfers.count - routes.count
            + state.nodes.count - nodes.count
            + source.sourcePulseCount - pulses.count
            + source.artifactCohorts.count - artifactCohorts.count
        reconciliation = RenderReconciliation(
            sourceNodes: state.nodes.count,
            visibleNodes: nodes.count,
            sourcePhysicalLinks: state.edges.count,
            visiblePhysicalLinks: physicalLinks.count,
            sourceParentRelations: sourceParents,
            visibleParentRelations: parentRelations.count,
            sourceRoutes: state.applicationTransfers.count,
            visibleRoutes: routes.count,
            sourceTransmissions: state.transmissions.count,
            visibleTransmissions: transmissions.count,
            sourcePulses: source.sourcePulseCount,
            visiblePulses: pulses.count,
            sourceArtifactCohorts: source.artifactCohorts.count,
            visibleArtifactCohorts: artifactCohorts.count,
            intentionallyOmitted: omitted,
            violations: source.violations
        )
    }

    func positions(in size: CGSize) -> [Int: CGPoint] {
        let viewport = WorldViewport(points: nodes.map(\.worldPoint), in: size)
        return Dictionary(uniqueKeysWithValues: nodes.map {
            ($0.state.id, viewport.project($0.worldPoint))
        })
    }

}
