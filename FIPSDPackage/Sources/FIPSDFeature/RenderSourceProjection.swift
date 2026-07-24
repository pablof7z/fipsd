import Foundation

struct RenderSourceProjection {
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
    let representedNodes: UInt64
    let sourceActiveNodes: Int
    let sourcePulseCount: Int
    let violations: [String]

    init(
        state: SimulationState,
        virtualTimeNS: UInt64,
        cohortState: CohortArtifactState?
    ) {
        nodes = state.nodes.values.sorted { $0.id < $1.id }.map {
            RenderNode(
                state: $0,
                worldPoint: RenderStableLayout.point(for: $0.id)
            )
        }
        physicalLinks = state.edges.values
            .filter { state.nodes[$0.from] != nil && state.nodes[$0.to] != nil }
            .sorted { $0.id < $1.id }
            .map(RenderPhysicalLink.init)
        parentRelations = state.nodes.values.compactMap { node in
            guard let parent = node.parent, state.nodes[parent] != nil else {
                return nil
            }
            return RenderParentRelation(child: node.id, parent: parent)
        }.sorted { ($0.child, $0.parent) < ($1.child, $1.parent) }
        routes = state.applicationTransfers.values
            .filter {
                !$0.path.isEmpty && $0.path.allSatisfy {
                    state.nodes[$0] != nil
                }
            }
            .sorted { $0.id < $1.id }
            .map(RenderRoute.init)
        transmissions = state.transmissions.values
            .filter { state.nodes[$0.from] != nil && state.nodes[$0.to] != nil }
            .sorted { $0.id < $1.id }
            .map {
                let span = max(1, $0.endNS - $0.startNS)
                let elapsed = virtualTimeNS > $0.startNS
                    ? virtualTimeNS - $0.startNS
                    : 0
                return RenderTransmission(
                    transmission: $0,
                    progress: min(1, Double(elapsed) / Double(span))
                )
            }
        pulses = Self.activePulses(state: state, virtualTimeNS: virtualTimeNS)
        let cohortProjection = CohortProjection(
            nodes: nodes,
            transmissions: transmissions
        )
        cohorts = cohortProjection.cohorts
        cohortTransmissions = cohortProjection.transmissions
        artifactCohorts = cohortState?.cohorts
            .map(RenderArtifactCohort.init)
            .sorted { $0.id < $1.id } ?? []
        artifactCohortFidelity = cohortState?.fidelity
        representedNodes = cohortState?.representedNodes
            ?? UInt64(state.nodes.count)
        sourceActiveNodes = state.nodes.values.count(where: \.active)
        sourcePulseCount = state.lastRekeyAtNS.count
            + state.lastParentSwitchAtNS.count
            + state.lastSybilArrivalAtNS.count
        violations = Self.sourceViolations(state)
    }

    private static func activePulses(
        state: SimulationState,
        virtualTimeNS: UInt64
    ) -> [RenderPulse] {
        let sources: [(RenderPulseKind, [Int: UInt64])] = [
            (.rekey, state.lastRekeyAtNS),
            (.parentSwitch, state.lastParentSwitchAtNS),
            (.authenticatedSybilArrival, state.lastSybilArrivalAtNS)
        ]
        return sources.flatMap { kind, values in
            values.compactMap { nodeID, time -> RenderPulse? in
                guard state.nodes[nodeID] != nil, virtualTimeNS >= time else {
                    return nil
                }
                let age = virtualTimeNS - time
                guard age <= kind.durationNS else { return nil }
                return RenderPulse(
                    nodeID: nodeID,
                    kind: kind,
                    occurredAtNS: time,
                    progress: Double(age) / Double(kind.durationNS)
                )
            }
        }.sorted {
            ($0.nodeID, $0.kind.rawValue, $0.occurredAtNS)
                < ($1.nodeID, $1.kind.rawValue, $1.occurredAtNS)
        }
    }

    private static func sourceViolations(
        _ state: SimulationState
    ) -> [String] {
        var result: [String] = []
        let missingEdges = state.edges.values.count {
            state.nodes[$0.from] == nil || state.nodes[$0.to] == nil
        }
        let missingFlights = state.transmissions.values.count {
            state.nodes[$0.from] == nil || state.nodes[$0.to] == nil
        }
        let missingParents = state.nodes.values.count {
            $0.parent.map { state.nodes[$0] == nil } ?? false
        }
        let missingRoutes = state.applicationTransfers.values.count { transfer in
            transfer.path.contains { state.nodes[$0] == nil }
        }
        let pulseNodeIDs = Set(state.lastRekeyAtNS.keys)
            .union(state.lastParentSwitchAtNS.keys)
            .union(state.lastSybilArrivalAtNS.keys)
        let missingPulses = pulseNodeIDs.count {
            state.nodes[$0] == nil
        }
        if missingEdges > 0 {
            result.append("\(missingEdges) links lack endpoints")
        }
        if missingFlights > 0 {
            result.append("\(missingFlights) transmissions lack endpoints")
        }
        if missingParents > 0 {
            result.append("\(missingParents) parent relations lack endpoints")
        }
        if missingRoutes > 0 {
            result.append("\(missingRoutes) application routes lack nodes")
        }
        if missingPulses > 0 {
            result.append("\(missingPulses) pulse sources lack nodes")
        }
        return result
    }
}
