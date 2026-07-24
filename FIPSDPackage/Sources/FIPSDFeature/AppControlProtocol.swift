import Foundation

struct AppControlRequest: Codable, Sendable {
    let id: String
    let token: String
    let command: String
    let arguments: [String: JSONValue]
}

struct AppControlResponse: Codable, Sendable {
    let id: String
    let ok: Bool
    let result: JSONValue?
    let error: String?

    static func success(_ id: String, _ result: JSONValue) -> Self {
        Self(id: id, ok: true, result: result, error: nil)
    }

    static func failure(_ id: String, _ error: String) -> Self {
        Self(id: id, ok: false, result: nil, error: error)
    }
}

struct AppControlEndpoint: Codable, Sendable {
    let version: Int
    let pid: Int32
    let port: UInt16
    let token: String

    static var fileURL: URL {
        FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        )[0]
        .appendingPathComponent("FIPSD", isDirectory: true)
        .appendingPathComponent("control-endpoint.json")
    }
}

enum AppControlError: LocalizedError {
    case invalidRequest
    case unsupportedCommand(String)
    case invalidArgument(String)
    case noActiveCampaign

    var errorDescription: String? {
        switch self {
        case .invalidRequest: "The control request is invalid."
        case let .unsupportedCommand(command): "Unsupported control command \(command)."
        case let .invalidArgument(argument): "Invalid control argument \(argument)."
        case .noActiveCampaign: "No active Campaign is available."
        }
    }
}
