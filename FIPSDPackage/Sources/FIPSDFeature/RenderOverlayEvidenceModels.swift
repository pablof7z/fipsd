import Foundation

struct RenderPulseEvidence: Codable, Equatable, Sendable {
    let nodeID: Int
    let kind: RenderPulseKind
    let occurredAtNS: UInt64
    let progress: Double
    let source: String

    init(_ pulse: RenderPulse) {
        nodeID = pulse.nodeID
        kind = pulse.kind
        occurredAtNS = pulse.occurredAtNS
        progress = pulse.progress
        let field = switch pulse.kind {
        case .rekey: "lastRekeyAtNS"
        case .parentSwitch: "lastParentSwitchAtNS"
        case .authenticatedSybilArrival: "lastSybilArrivalAtNS"
        }
        source = "state.\(field)[\(pulse.nodeID)]"
    }

    enum CodingKeys: String, CodingKey {
        case nodeID = "node_id"
        case kind
        case occurredAtNS = "occurred_at_ns"
        case progress
        case source
    }
}

struct RenderArtifactCohortEvidence: Codable, Equatable, Sendable {
    let id: String
    let population: UInt64
    let depthStart: UInt64
    let depthEnd: UInt64
    let region: String
    let transport: String
    let resource: String
    let protocolState: String
    let worldX: Double
    let worldY: Double
    let source: String

    init(_ cohort: RenderArtifactCohort) {
        id = cohort.id
        population = cohort.population
        depthStart = cohort.depthStart
        depthEnd = cohort.depthEnd
        region = cohort.region
        transport = cohort.transport
        resource = cohort.resource
        protocolState = cohort.protocolState
        worldX = cohort.worldPoint.x
        worldY = cohort.worldPoint.y
        source = "artifact.samples[0].cohorts[id=\(cohort.id)]"
    }

    enum CodingKeys: String, CodingKey {
        case id
        case population
        case depthStart = "depth_start"
        case depthEnd = "depth_end"
        case region
        case transport
        case resource
        case protocolState = "protocol_state"
        case worldX = "world_x"
        case worldY = "world_y"
        case source
    }
}
