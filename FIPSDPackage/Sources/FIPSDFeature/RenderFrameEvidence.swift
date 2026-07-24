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
    let sourcePulses: Int
    let visiblePulses: Int
    let sourceArtifactCohorts: Int
    let visibleArtifactCohorts: Int
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
        sourcePulses = value.sourcePulses
        visiblePulses = value.visiblePulses
        sourceArtifactCohorts = value.sourceArtifactCohorts
        visibleArtifactCohorts = value.visibleArtifactCohorts
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
    let startedPulseKeys: [String]
    let endedPulseKeys: [String]
    let advancedPulseKeys: [String]
    let changedCohorts: [String]
    let changedArtifactCohortIDs: [String]
    let layoutOnlyNodeIDs: [Int]
    let viewModeChanged: Bool
    let anomalyFilterChanged: Bool
    let selectionChanged: Bool

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
            startedPulseKeys = current.pulses.map(Self.pulseKey)
            endedPulseKeys = []
            advancedPulseKeys = []
            changedCohorts = current.cohorts.map(Self.cohortLabel)
            changedArtifactCohortIDs = current.artifactCohorts.map(\.id)
            layoutOnlyNodeIDs = []
            viewModeChanged = false
            anomalyFilterChanged = false
            selectionChanged = false
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
        let beforePulses = Dictionary(
            uniqueKeysWithValues: previous.pulses.map {
                (Self.pulseKey($0), $0)
            }
        )
        let afterPulses = Dictionary(
            uniqueKeysWithValues: current.pulses.map {
                (Self.pulseKey($0), $0)
            }
        )
        startedPulseKeys = afterPulses.keys
            .filter { beforePulses[$0] == nil }.sorted()
        endedPulseKeys = beforePulses.keys
            .filter { afterPulses[$0] == nil }.sorted()
        advancedPulseKeys = afterPulses.compactMap { key, pulse in
            guard let before = beforePulses[key],
                  before.occurredAtNS == pulse.occurredAtNS,
                  before.progress != pulse.progress else { return nil }
            return key
        }.sorted()
        changedCohorts = Self.changedKeys(
            previous.cohorts,
            current.cohorts,
            key: Self.cohortLabel
        )
        changedArtifactCohortIDs = Self.changedKeys(
            previous.artifactCohorts,
            current.artifactCohorts,
            key: \.id
        )
        viewModeChanged = previous.visualizationMode
            != current.visualizationMode
        anomalyFilterChanged = previous.anomalyNodeIDs
            != current.anomalyNodeIDs
        selectionChanged = previous.selectedNodeID != current.selectedNodeID
    }

    var eventAttributedChangeCount: Int {
        addedNodeIDs.count + removedNodeIDs.count + changedNodeIDs.count
            + changedPhysicalLinkIDs.count + changedParentNodeIDs.count
            + changedRouteIDs.count + startedTransmissionIDs.count
            + startedPulseKeys.count + changedCohorts.count
            + changedArtifactCohortIDs.count
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

    private static func pulseKey(_ pulse: RenderPulse) -> String {
        "\(pulse.nodeID):\(pulse.kind.rawValue)"
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
        case startedPulseKeys = "started_pulse_keys"
        case endedPulseKeys = "ended_pulse_keys"
        case advancedPulseKeys = "advanced_pulse_keys"
        case changedCohorts = "changed_cohorts"
        case changedArtifactCohortIDs = "changed_artifact_cohort_ids"
        case layoutOnlyNodeIDs = "layout_only_node_ids"
        case viewModeChanged = "view_mode_changed"
        case anomalyFilterChanged = "anomaly_filter_changed"
        case selectionChanged = "selection_changed"
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
        previous: RenderFrame?
    ) {
        schema = "experiments.fips.network/render-frame/v1alpha1"
        self.frameIndex = frameIndex
        virtualTimeNS = frame.virtualTimeNS
        presentation = RenderPresentationEvidence(
            frame.displayBatch,
            visualizationMode: frame.visualizationMode,
            anomalyNodeIDs: frame.anomalyNodeIDs,
            selectedNodeID: frame.selectedNodeID
        )
        fidelity = RenderFidelityEvidence(
            sourceFidelity: frame.sourceFidelity,
            batch: frame.displayBatch
        )
        primitives = RenderPrimitiveEvidence(frame)
        reconciliation = RenderReconciliationEvidence(frame.reconciliation)
        delta = RenderFrameDeltaEvidence(previous: previous, current: frame)
        var problems = frame.reconciliation.violations
        if !delta.layoutOnlyNodeIDs.isEmpty {
            problems.append("node world coordinates changed without source-state changes")
        }
        if previous != nil, delta.eventAttributedChangeCount > 0,
           frame.displayBatch.eventIDs.isEmpty,
           frame.displayBatch.mode != .viewChange {
            problems.append("structural visible changes lack ordered-event attribution")
        }
        if frame.displayBatch.mode == .viewChange,
           !frame.displayBatch.eventIDs.isEmpty {
            problems.append("view-change presentation contains simulation events")
        }
        let order = zip(
            zip(frame.displayBatch.eventTimesNS, frame.displayBatch.eventOrdinals),
            zip(
                frame.displayBatch.eventTimesNS.dropFirst(),
                frame.displayBatch.eventOrdinals.dropFirst()
            )
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
