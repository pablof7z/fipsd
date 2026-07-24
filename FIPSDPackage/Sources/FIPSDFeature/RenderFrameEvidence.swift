import Foundation

struct RenderReconciliationEvidence: Codable, Equatable, Sendable {
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

    init(_ value: RenderReconciliation) {
        sourceNodes = value.sourceNodes
        visibleNodes = value.visibleNodes
        sourcePhysicalLinks = value.sourcePhysicalLinks
        visiblePhysicalLinks = value.visiblePhysicalLinks
        sourceParentRelations = value.sourceParentRelations
        visibleParentRelations = value.visibleParentRelations
        sourceRoutes = value.sourceRoutes
        visibleRoutes = value.visibleRoutes
        sourceTransmissions = value.sourceTransmissions
        visibleTransmissions = value.visibleTransmissions
        intentionallyOmitted = value.intentionallyOmitted
        violations = value.violations
    }
}

struct RenderFrameDeltaEvidence: Codable, Equatable, Sendable {
    let addedNodeIDs: [Int]
    let removedNodeIDs: [Int]
    let changedNodeIDs: [Int]
    let changedPhysicalLinkIDs: [Int]
    let changedParentNodeIDs: [Int]
    let changedRouteIDs: [String]
    let startedTransmissionIDs: [String]
    let endedTransmissionIDs: [String]
    let advancedTransmissionIDs: [String]
    let changedCohorts: [String]
    let layoutOnlyNodeIDs: [Int]

    init(previous: RenderFrame?, current: RenderFrame) {
        guard let previous else {
            addedNodeIDs = current.nodes.map(\.state.id)
            removedNodeIDs = []
            changedNodeIDs = []
            changedPhysicalLinkIDs = current.physicalLinks.map(\.edge.id)
            changedParentNodeIDs = current.parentRelations.map(\.child)
            changedRouteIDs = current.routes.map(\.transferID)
            startedTransmissionIDs = current.transmissions.map(\.transmission.id)
            endedTransmissionIDs = []
            advancedTransmissionIDs = []
            changedCohorts = current.cohorts.map(Self.cohortLabel)
            layoutOnlyNodeIDs = []
            return
        }
        let beforeNodes = Dictionary(
            uniqueKeysWithValues: previous.nodes.map { ($0.state.id, $0) }
        )
        let afterNodes = Dictionary(
            uniqueKeysWithValues: current.nodes.map { ($0.state.id, $0) }
        )
        addedNodeIDs = afterNodes.keys.filter { beforeNodes[$0] == nil }.sorted()
        removedNodeIDs = beforeNodes.keys.filter { afterNodes[$0] == nil }.sorted()
        changedNodeIDs = afterNodes.compactMap { id, node in
            guard let before = beforeNodes[id], before.state != node.state else { return nil }
            return id
        }.sorted()
        layoutOnlyNodeIDs = afterNodes.compactMap { id, node in
            guard let before = beforeNodes[id],
                  before.state == node.state,
                  before.worldPoint != node.worldPoint else { return nil }
            return id
        }.sorted()
        changedPhysicalLinkIDs = Self.changedKeys(
            previous.physicalLinks,
            current.physicalLinks,
            key: \.edge.id
        )
        changedParentNodeIDs = Self.changedKeys(
            previous.parentRelations,
            current.parentRelations,
            key: \.child
        )
        changedRouteIDs = Self.changedKeys(
            previous.routes,
            current.routes,
            key: \.transferID
        )
        let beforeFlights = Dictionary(
            uniqueKeysWithValues: previous.transmissions.map {
                ($0.transmission.id, $0)
            }
        )
        let afterFlights = Dictionary(
            uniqueKeysWithValues: current.transmissions.map {
                ($0.transmission.id, $0)
            }
        )
        startedTransmissionIDs = afterFlights.keys
            .filter { beforeFlights[$0] == nil }.sorted()
        endedTransmissionIDs = beforeFlights.keys
            .filter { afterFlights[$0] == nil }.sorted()
        advancedTransmissionIDs = afterFlights.compactMap { id, flight in
            guard let before = beforeFlights[id],
                  before.transmission == flight.transmission,
                  before.progress != flight.progress else { return nil }
            return id
        }.sorted()
        changedCohorts = Self.changedKeys(
            previous.cohorts,
            current.cohorts,
            key: Self.cohortLabel
        )
    }

    var eventAttributedChangeCount: Int {
        addedNodeIDs.count + removedNodeIDs.count + changedNodeIDs.count
            + changedPhysicalLinkIDs.count + changedParentNodeIDs.count
            + changedRouteIDs.count + startedTransmissionIDs.count
            + changedCohorts.count
    }

    private static func changedKeys<Element: Equatable, Key: Hashable & Comparable>(
        _ before: [Element],
        _ after: [Element],
        key: (Element) -> Key
    ) -> [Key] {
        let left = Dictionary(uniqueKeysWithValues: before.map { (key($0), $0) })
        let right = Dictionary(uniqueKeysWithValues: after.map { (key($0), $0) })
        return Set(left.keys).union(right.keys).filter {
            left[$0] != right[$0]
        }.sorted()
    }

    private static func cohortLabel(_ cohort: RenderCohort) -> String {
        let key = cohort.key
        return "\(key.root):\(key.depthBand):\(key.transport)"
    }

    enum CodingKeys: String, CodingKey {
        case addedNodeIDs = "added_node_ids"
        case removedNodeIDs = "removed_node_ids"
        case changedNodeIDs = "changed_node_ids"
        case changedPhysicalLinkIDs = "changed_physical_link_ids"
        case changedParentNodeIDs = "changed_parent_node_ids"
        case changedRouteIDs = "changed_route_ids"
        case startedTransmissionIDs = "started_transmission_ids"
        case endedTransmissionIDs = "ended_transmission_ids"
        case advancedTransmissionIDs = "advanced_transmission_ids"
        case changedCohorts = "changed_cohorts"
        case layoutOnlyNodeIDs = "layout_only_node_ids"
    }
}

struct RenderFrameEvidence: Codable, Equatable, Sendable {
    let schema: String
    let frameIndex: Int
    let virtualTimeNS: UInt64
    let presentation: RenderPresentationEvidence
    let fidelity: RenderFidelityEvidence
    let primitives: RenderPrimitiveEvidence
    let reconciliation: RenderReconciliationEvidence
    let delta: RenderFrameDeltaEvidence
    let violations: [String]

    init(
        frameIndex: Int,
        frame: RenderFrame,
        previous: RenderFrame?,
        batch: DisplayProjectionBatch,
        sourceFidelity: String
    ) {
        schema = "experiments.fips.network/render-frame/v1alpha1"
        self.frameIndex = frameIndex
        virtualTimeNS = frame.virtualTimeNS
        presentation = RenderPresentationEvidence(batch)
        fidelity = RenderFidelityEvidence(
            sourceFidelity: sourceFidelity,
            batch: batch
        )
        primitives = RenderPrimitiveEvidence(frame)
        reconciliation = RenderReconciliationEvidence(frame.reconciliation)
        delta = RenderFrameDeltaEvidence(previous: previous, current: frame)
        var problems = frame.reconciliation.violations
        if !delta.layoutOnlyNodeIDs.isEmpty {
            problems.append("node world coordinates changed without source-state changes")
        }
        if previous != nil, delta.eventAttributedChangeCount > 0,
           batch.eventIDs.isEmpty {
            problems.append("structural visible changes lack ordered-event attribution")
        }
        let order = zip(
            zip(batch.eventTimesNS, batch.eventOrdinals),
            zip(batch.eventTimesNS.dropFirst(), batch.eventOrdinals.dropFirst())
        )
        if !order.allSatisfy({ pair in
            let before = pair.0
            let after = pair.1
            return before.0 < after.0
                || (before.0 == after.0 && before.1 < after.1)
        }) {
            problems.append("display update event ordinals are not strictly increasing")
        }
        violations = problems
    }
}
