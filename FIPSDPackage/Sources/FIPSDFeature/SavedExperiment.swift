import Foundation

struct SavedExperiment: Codable, Equatable, Sendable {
    let version: Int
    let id: String
    let name: String
    let description: String?
    let savedAt: Date
    let campaignSHA256: String
    let sourceRunID: String?
    let declaredFidelity: JSONValue
    let sourceResultFidelity: String?

    var controlValue: JSONValue {
        .object([
            "id": .string(id),
            "name": .string(name),
            "description": description.map(JSONValue.string) ?? .null,
            "saved_at": .string(ISO8601DateFormatter().string(from: savedAt)),
            "campaign_sha256": .string(campaignSHA256),
            "source_run_id": sourceRunID.map(JSONValue.string) ?? .null,
            "declared_fidelity": declaredFidelity,
            "source_result_fidelity": sourceResultFidelity
                .map(JSONValue.string) ?? .null
        ])
    }
}

enum ExperimentStoreError: LocalizedError {
    case invalidCampaign
    case invalidName
    case invalidIdentifier
    case notFound(String)
    case corrupt(String)

    var errorDescription: String? {
        switch self {
        case .invalidCampaign:
            "The active Campaign is not a JSON object."
        case .invalidName:
            "The saved experiment name must not be empty."
        case .invalidIdentifier:
            "The saved experiment identifier is invalid."
        case let .notFound(id):
            "No saved experiment exists with identifier \(id)."
        case let .corrupt(id):
            "Saved experiment \(id) failed its Campaign checksum."
        }
    }
}
