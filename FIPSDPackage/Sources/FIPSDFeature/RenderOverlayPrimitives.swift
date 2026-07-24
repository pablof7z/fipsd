import Foundation

enum RenderPulseKind: String, Codable, Equatable, Sendable {
    case rekey
    case parentSwitch = "parent-switch"
    case authenticatedSybilArrival = "authenticated-sybil-arrival"

    var durationNS: UInt64 {
        switch self {
        case .rekey: 250_000_000
        case .parentSwitch: 350_000_000
        case .authenticatedSybilArrival: 500_000_000
        }
    }
}

struct RenderPulse: Equatable, Sendable {
    let nodeID: Int
    let kind: RenderPulseKind
    let occurredAtNS: UInt64
    let progress: Double
}

struct RenderArtifactCohort: Equatable, Sendable {
    let id: String
    let population: UInt64
    let depthStart: UInt64
    let depthEnd: UInt64
    let region: String
    let transport: String
    let resource: String
    let protocolState: String
    let worldPoint: RenderWorldPoint

    init(_ record: CohortRecord) {
        id = record.id
        population = record.population
        depthStart = record.depthStart
        depthEnd = record.depthEnd
        region = record.region
        transport = record.transport
        resource = record.resource
        protocolState = record.protocolState
        worldPoint = RenderStableLayout.point(for: record.id)
    }
}

enum RenderStableLayout {
    static func point(for nodeID: Int) -> RenderWorldPoint {
        point(seed: UInt64(bitPattern: Int64(nodeID)))
    }

    static func point(for identifier: String) -> RenderWorldPoint {
        var value: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in identifier.utf8 {
            value ^= UInt64(byte)
            value &*= 0x0000_0100_0000_01b3
        }
        return point(seed: value)
    }

    private static func point(seed: UInt64) -> RenderWorldPoint {
        var value = seed &+ 0x9E37_79B9_7F4A_7C15
        value = (value ^ (value >> 30)) &* 0xBF58_476D_1CE4_E5B9
        value = (value ^ (value >> 27)) &* 0x94D0_49BB_1331_11EB
        value ^= value >> 31
        let radius = 0.12 + 0.86 * Double(value & 0xFFFF) / 65_535
        let angle = Double((value >> 16) & 0xFFFF_FFFF)
            / Double(UInt32.max) * 2 * .pi
        return RenderWorldPoint(
            x: Foundation.cos(angle) * radius,
            y: Foundation.sin(angle) * radius
        )
    }
}
