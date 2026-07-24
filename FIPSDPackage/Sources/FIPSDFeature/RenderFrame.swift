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
    let transferID: String
    let nodeIDs: [Int]
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
    let intentionallyOmitted: Int
    let violations: [String]

    var isExact: Bool { violations.isEmpty }
}

struct RenderFrame: Equatable, Sendable {
    let virtualTimeNS: UInt64
    let nodes: [RenderNode]
    let physicalLinks: [RenderPhysicalLink]
    let parentRelations: [RenderParentRelation]
    let routes: [RenderRoute]
    let transmissions: [RenderTransmission]
    let cohorts: [RenderCohort]
    let cohortTransmissions: [CohortFlightAggregate]
    let reconciliation: RenderReconciliation

    init(
        state: SimulationState,
        virtualTimeNS: UInt64,
        visibleNodeIDs: Set<Int>? = nil
    ) {
        self.virtualTimeNS = virtualTimeNS
        let included = visibleNodeIDs ?? Set(state.nodes.keys)
        nodes = state.nodes.values
            .filter { included.contains($0.id) }
            .sorted { $0.id < $1.id }
            .map { RenderNode(state: $0, worldPoint: Self.stablePoint(for: $0.id)) }
        physicalLinks = state.edges.values
            .filter { included.contains($0.from) && included.contains($0.to) }
            .sorted { $0.id < $1.id }
            .map(RenderPhysicalLink.init)
        parentRelations = state.nodes.values
            .compactMap { node -> RenderParentRelation? in
                guard included.contains(node.id), let parent = node.parent,
                      included.contains(parent), state.nodes[parent] != nil else { return nil }
                return RenderParentRelation(child: node.id, parent: parent)
            }
            .sorted { ($0.child, $0.parent) < ($1.child, $1.parent) }
        routes = state.applicationTransfers.values
            .filter {
                !$0.path.isEmpty && $0.path.allSatisfy(included.contains)
            }
            .sorted { $0.id < $1.id }
            .map {
                RenderRoute(transferID: $0.id, nodeIDs: $0.path)
            }
        transmissions = state.transmissions.values
            .filter { included.contains($0.from) && included.contains($0.to) }
            .sorted { $0.id < $1.id }
            .map {
                let span = max(1, $0.endNS - $0.startNS)
                let elapsed = virtualTimeNS > $0.startNS ? virtualTimeNS - $0.startNS : 0
                return RenderTransmission(
                    transmission: $0,
                    progress: min(1, Double(elapsed) / Double(span))
                )
            }
        let cohortProjection = CohortProjection(
            nodes: nodes,
            transmissions: transmissions
        )
        cohorts = cohortProjection.cohorts
        cohortTransmissions = cohortProjection.transmissions
        var violations: [String] = []
        let missingEdges = state.edges.values.filter {
            state.nodes[$0.from] == nil || state.nodes[$0.to] == nil
        }.count
        let missingFlights = state.transmissions.values.filter {
            state.nodes[$0.from] == nil || state.nodes[$0.to] == nil
        }.count
        let missingParents = state.nodes.values.filter {
            guard let parent = $0.parent else { return false }
            return state.nodes[parent] == nil
        }.count
        let missingRoutes = state.applicationTransfers.values.filter { transfer in
            transfer.path.contains { state.nodes[$0] == nil }
        }.count
        if missingEdges > 0 { violations.append("\(missingEdges) links lack endpoints") }
        if missingFlights > 0 {
            violations.append("\(missingFlights) transmissions lack endpoints")
        }
        if missingParents > 0 {
            violations.append("\(missingParents) parent relations lack endpoints")
        }
        if missingRoutes > 0 {
            violations.append("\(missingRoutes) application routes lack nodes")
        }
        let sourceParents = state.nodes.values.filter { $0.parent != nil }.count
        let omitted = state.transmissions.count - transmissions.count
            + state.edges.count - physicalLinks.count
            + sourceParents - parentRelations.count
            + state.applicationTransfers.count - routes.count
            + state.nodes.count - nodes.count
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
            intentionallyOmitted: omitted,
            violations: violations
        )
    }

    func positions(in size: CGSize) -> [Int: CGPoint] {
        Dictionary(uniqueKeysWithValues: nodes.map {
            ($0.state.id, $0.worldPoint.projected(in: size))
        })
    }

    private static func stablePoint(for id: Int) -> RenderWorldPoint {
        var value = UInt64(bitPattern: Int64(id)) &+ 0x9E37_79B9_7F4A_7C15
        value = (value ^ (value >> 30)) &* 0xBF58_476D_1CE4_E5B9
        value = (value ^ (value >> 27)) &* 0x94D0_49BB_1331_11EB
        value ^= value >> 31
        let radius = 0.12 + 0.86 * Double(value & 0xFFFF) / 65_535
        let angle = Double((value >> 16) & 0xFFFF_FFFF)
            / Double(UInt32.max) * 2 * .pi
        return RenderWorldPoint(x: cos(angle) * radius, y: sin(angle) * radius)
    }
}
