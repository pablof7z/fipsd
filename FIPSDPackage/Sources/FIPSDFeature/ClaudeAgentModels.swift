import Foundation

enum ClaudeConnectionState: Equatable, Sendable {
    case disconnected
    case connecting
    case ready
    case responding
    case failed(String)

    var label: String {
        switch self {
        case .disconnected: "Offline"
        case .connecting: "Connecting"
        case .ready: "Ready"
        case .responding: "Working"
        case .failed: "Unavailable"
        }
    }

    var isConnected: Bool {
        switch self {
        case .ready, .responding: true
        default: false
        }
    }
}

enum ClaudeTranscriptRole: Equatable, Sendable {
    case user
    case agent
    case activity
    case notice
    case failure
}

struct ClaudeTranscriptEntry: Identifiable, Equatable, Sendable {
    let id: String
    var role: ClaudeTranscriptRole
    var text: String
    var detail: String?

    init(
        id: String = UUID().uuidString,
        role: ClaudeTranscriptRole,
        text: String,
        detail: String? = nil
    ) {
        self.id = id
        self.role = role
        self.text = text
        self.detail = detail
    }
}

struct ClaudePermissionOption: Identifiable, Equatable, Sendable {
    let id: String
    let name: String
    let kind: String
}

struct ClaudePermissionRequest: Identifiable, Equatable, Sendable {
    let rpcID: JSONValue
    let toolCallID: String
    let title: String
    let options: [ClaudePermissionOption]

    var id: String { rpcID.prettyDescription }
}

enum ClaudeACPEvent: Equatable, Sendable {
    case message(id: String?, text: String)
    case activity(id: String, title: String, status: String?)
    case usage(used: Int, size: Int)
    case diagnostic(String)
    case exited(String?)
}

enum ClaudeACPError: LocalizedError, Equatable {
    case missingNPX
    case missingMCP(String)
    case missingSkill(String)
    case processLaunch(String)
    case processExited(String?)
    case invalidResponse(String)
    case rpc(code: Int, message: String)
    case timeout(String)
    case notConnected

    var errorDescription: String? {
        switch self {
        case .missingNPX:
            "npx was not found. Install Node.js so Claude's ACP adapter can run."
        case let .missingMCP(path):
            "The Wind Tunnel MCP server is not installed at \(path)."
        case let .missingSkill(path):
            "The Wind Tunnel operating skill is unavailable at \(path)."
        case let .processLaunch(message):
            "Claude ACP could not start: \(message)"
        case let .processExited(message):
            "Claude ACP exited\(message.map { ": \($0)" } ?? ".")"
        case let .invalidResponse(message):
            "Claude ACP returned an invalid response: \(message)"
        case let .rpc(code, message):
            "Claude ACP error \(code): \(message)"
        case let .timeout(method):
            "Claude ACP timed out during \(method)."
        case .notConnected:
            "Claude ACP is not connected."
        }
    }
}
