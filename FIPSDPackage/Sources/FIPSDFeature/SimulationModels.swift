import Foundation

struct StreamEnvelope: Decodable, Sendable {
    let apiVersion: String
    let kind: String
    let payload: JSONValue
    let runID: String?

    enum CodingKeys: String, CodingKey {
        case apiVersion = "api_version"
        case kind, payload
        case runID = "run_id"
    }
}

struct SimulationEvent: Identifiable, Equatable, Sendable {
    let id: String
    let timeNS: UInt64
    let ordinal: UInt64
    let kind: String
    let causalParent: String?
    let data: [String: JSONValue]

    init?(_ payload: JSONValue) {
        guard let object = payload.object,
              let id = object["event_id"]?.string,
              let time = object["virtual_time_ns"]?.uint64,
              let ordinal = object["ordinal"]?.uint64,
              let kind = object["kind"]?.string,
              let data = object["data"]?.object else { return nil }
        self.id = id
        timeNS = time
        self.ordinal = ordinal
        self.kind = kind
        causalParent = object["causal_parent"]?.string
        self.data = data
    }
}

struct NodeState: Identifiable, Equatable, Sendable {
    let id: Int
    var address: String
    var active: Bool
    var root: Int
    var parent: Int?
    var sequence: Int
    var transportProfile = "udp"
    var transportType = "udp"
    var bandwidthBPS = 0
    var latencyNS: UInt64 = 0
    var jitterNS: UInt64 = 0
    var mtuBytes = 0
    var mediaZone: String?
}

struct EdgeState: Identifiable, Equatable, Sendable {
    let id: Int
    let from: Int
    let to: Int
    var active = true
    var bandwidthBPS = 0
    var latencyNS: UInt64 = 0
    var jitterNS: UInt64 = 0
    var lossPPM = 0
    var mtuBytes = 0
    var queueBytes = 0
    var sharedMediumGroup: Int?
    var parentCostPPM = 1_000_000
}

struct Transmission: Identifiable, Equatable, Sendable {
    let id: String
    let from: Int
    let to: Int
    let startNS: UInt64
    let endNS: UInt64
    let frameBytes: Int
    let copy: Int
    let plane: String
}

struct RunSummary: Equatable, Sendable {
    var runID = ""
    var artifactID = ""
    var finalRoot = ""
    var quiescenceNS: UInt64 = 0
    var fidelity = "Awaiting run evidence"
    var outcome = ""
}

enum AuthoringProvider: String, CaseIterable, Identifiable, Sendable {
    case automatic = "Auto"
    case claudeSonnet = "Claude Sonnet"
    case claudeHaiku = "Claude Haiku"
    case claudeOpus = "Claude Opus"
    case codex = "Codex"

    var id: Self { self }

    var executableName: String? {
        switch self {
        case .automatic: nil
        case .claudeSonnet, .claudeHaiku, .claudeOpus: "claude"
        case .codex: "codex"
        }
    }

    var claudeModel: String? {
        switch self {
        case .claudeSonnet: "sonnet"
        case .claudeHaiku: "haiku"
        case .claudeOpus: "opus"
        case .automatic, .codex: nil
        }
    }
}

enum VisualizationMode: String, Codable, CaseIterable, Identifiable, Sendable {
    case cohorts = "Cohorts"
    case rootAdoption = "Root adoption"
    case connectivity = "Connectivity"
    case sharedMedium = "Shared media"
    case anomalies = "Anomalies"
    var id: Self { self }
}
