import Foundation

struct RenderPresentationEvidence: Codable, Equatable, Sendable {
    let mode: DisplayProjectionMode
    let fromNS: UInt64
    let throughNS: UInt64
    let eventIDs: [String]
    let eventKinds: [String]
    let eventTimesNS: [UInt64]
    let eventOrdinals: [UInt64]
    let causalParents: [String?]
    let initiatingEventIDs: [String]
    let compressionReason: DisplayCompressionReason?

    init(_ batch: DisplayProjectionBatch) {
        mode = batch.mode
        fromNS = batch.fromNS
        throughNS = batch.throughNS
        eventIDs = batch.eventIDs
        eventKinds = batch.eventKinds
        eventTimesNS = batch.eventTimesNS
        eventOrdinals = batch.eventOrdinals
        causalParents = batch.causalParents
        initiatingEventIDs = batch.initiatingEventIDs
        compressionReason = batch.compressionReason
    }

    enum CodingKeys: String, CodingKey {
        case mode
        case fromNS = "from_ns"
        case throughNS = "through_ns"
        case eventIDs = "event_ids"
        case eventKinds = "event_kinds"
        case eventTimesNS = "event_times_ns"
        case eventOrdinals = "event_ordinals"
        case causalParents = "causal_parents"
        case initiatingEventIDs = "initiating_event_ids"
        case compressionReason = "compression_reason"
    }
}

struct RenderFidelityEvidence: Codable, Equatable, Sendable {
    let sourceFidelity: String
    let visibleState: String
    let temporal: String
    let layout: String
    let cohorts: String

    init(sourceFidelity: String, batch: DisplayProjectionBatch) {
        self.sourceFidelity = sourceFidelity
        visibleState = "exact projection of retained SimulationState fields"
        temporal = batch.isCompressed
            ? "ordered events exactly summarized; no intermediate state claimed"
            : "ordered-event or virtual-time interpolation frame"
        layout = "stable synthetic world coordinates; distance is not a protocol metric"
        cohorts = "deterministic root-depth-transport aggregation"
    }
}

struct RenderNodeEvidence: Codable, Equatable, Sendable {
    let id: Int
    let active: Bool
    let root: Int
    let parent: Int?
    let sequence: Int
    let transport: String
    let worldX: Double
    let worldY: Double
    let source: String

    init(_ node: RenderNode) {
        id = node.state.id
        active = node.state.active
        root = node.state.root
        parent = node.state.parent
        sequence = node.state.sequence
        transport = node.state.transportType
        worldX = node.worldPoint.x
        worldY = node.worldPoint.y
        source = "state.nodes[\(id)]"
    }
}

struct RenderLinkEvidence: Codable, Equatable, Sendable {
    let id: Int
    let from: Int
    let to: Int
    let active: Bool
    let source: String

    init(_ link: RenderPhysicalLink) {
        id = link.edge.id
        from = link.edge.from
        to = link.edge.to
        active = link.edge.active
        source = "state.edges[\(id)]"
    }
}

struct RenderParentEvidence: Codable, Equatable, Sendable {
    let child: Int
    let parent: Int
    let source: String

    init(_ relation: RenderParentRelation) {
        child = relation.child
        parent = relation.parent
        source = "state.nodes[\(child)].parent"
    }
}

struct RenderRouteEvidence: Codable, Equatable, Sendable {
    let transferID: String
    let nodeIDs: [Int]
    let source: String

    init(_ route: RenderRoute) {
        transferID = route.transferID
        nodeIDs = route.nodeIDs
        source = "state.applicationTransfers[\(transferID)].path"
    }

    enum CodingKeys: String, CodingKey {
        case transferID = "transfer_id"
        case nodeIDs = "node_ids"
        case source
    }
}

struct RenderTransmissionEvidence: Codable, Equatable, Sendable {
    let id: String
    let from: Int
    let to: Int
    let startNS: UInt64
    let endNS: UInt64
    let plane: String
    let progress: Double
    let source: String

    init(_ item: RenderTransmission) {
        let flight = item.transmission
        id = flight.id
        from = flight.from
        to = flight.to
        startNS = flight.startNS
        endNS = flight.endNS
        plane = flight.plane
        progress = item.progress
        source = "state.transmissions[\(id)]"
    }
}

struct RenderCohortEvidence: Codable, Equatable, Sendable {
    let root: Int
    let depthBand: Int
    let transport: String
    let nodeIDs: [Int]
    let activeNodes: Int
    let worldX: Double
    let worldY: Double
    let source: String

    init(_ cohort: RenderCohort) {
        root = cohort.key.root
        depthBand = cohort.key.depthBand
        transport = cohort.key.transport
        nodeIDs = cohort.nodeIDs
        activeNodes = cohort.activeNodes
        worldX = cohort.worldPoint.x
        worldY = cohort.worldPoint.y
        source = "derived(state.nodes: root × depth-band × transport)"
    }

    enum CodingKeys: String, CodingKey {
        case root
        case depthBand = "depth_band"
        case transport
        case nodeIDs = "node_ids"
        case activeNodes = "active_nodes"
        case worldX = "world_x"
        case worldY = "world_y"
        case source
    }
}

struct RenderCohortTransmissionEvidence: Codable, Equatable, Sendable {
    let from: String
    let to: String
    let plane: String
    let count: Int
    let meanProgress: Double
    let source: String

    init(_ aggregate: CohortFlightAggregate) {
        from = Self.label(aggregate.key.from)
        to = Self.label(aggregate.key.to)
        plane = aggregate.key.plane
        count = aggregate.count
        meanProgress = aggregate.meanProgress
        source = "derived(all matching state.transmissions)"
    }

    private static func label(_ key: CohortKey) -> String {
        "\(key.root):\(key.depthBand):\(key.transport)"
    }
}

struct RenderPrimitiveEvidence: Codable, Equatable, Sendable {
    let nodes: [RenderNodeEvidence]
    let physicalLinks: [RenderLinkEvidence]
    let parentRelations: [RenderParentEvidence]
    let routes: [RenderRouteEvidence]
    let transmissions: [RenderTransmissionEvidence]
    let cohorts: [RenderCohortEvidence]
    let cohortTransmissions: [RenderCohortTransmissionEvidence]

    init(_ frame: RenderFrame) {
        nodes = frame.nodes.map(RenderNodeEvidence.init)
        physicalLinks = frame.physicalLinks.map(RenderLinkEvidence.init)
        parentRelations = frame.parentRelations.map(RenderParentEvidence.init)
        routes = frame.routes.map(RenderRouteEvidence.init)
        transmissions = frame.transmissions.map(RenderTransmissionEvidence.init)
        cohorts = frame.cohorts.map(RenderCohortEvidence.init)
        cohortTransmissions = frame.cohortTransmissions.map(
            RenderCohortTransmissionEvidence.init
        )
    }
}
