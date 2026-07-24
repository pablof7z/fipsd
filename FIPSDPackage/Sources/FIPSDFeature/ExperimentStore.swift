import CryptoKit
import Foundation

final class ExperimentStore {
    private let rootURL: URL
    private let fileManager: FileManager

    init(
        rootURL: URL = ExperimentStore.defaultRootURL,
        fileManager: FileManager = .default
    ) {
        self.rootURL = rootURL
        self.fileManager = fileManager
    }

    func save(
        campaign: Data,
        name requestedName: String?,
        description requestedDescription: String?,
        sourceRunID: String?,
        sourceResultFidelity: String?
    ) throws -> SavedExperiment {
        let campaignObject = try campaignObject(campaign)
        let name = try resolvedName(requestedName, campaign: campaignObject)
        let description = resolvedDescription(
            requestedDescription,
            campaign: campaignObject
        )
        let id = UUID().uuidString.lowercased()
        let experiment = SavedExperiment(
            version: 1,
            id: id,
            name: name,
            description: description,
            savedAt: Date(
                timeIntervalSince1970: floor(Date().timeIntervalSince1970)
            ),
            campaignSHA256: sha256(campaign),
            sourceRunID: sourceRunID?.nilIfEmpty,
            declaredFidelity: try declaredFidelity(campaign),
            sourceResultFidelity: sourceResultFidelity?.nilIfEmpty
        )
        try fileManager.createDirectory(
            at: rootURL,
            withIntermediateDirectories: true
        )
        let stagingURL = rootURL.appendingPathComponent(
            ".saving-\(id)",
            isDirectory: true
        )
        let destinationURL = directory(for: id)
        try fileManager.createDirectory(
            at: stagingURL,
            withIntermediateDirectories: false
        )
        defer { try? fileManager.removeItem(at: stagingURL) }
        try campaign.write(
            to: stagingURL.appendingPathComponent("campaign.json"),
            options: .atomic
        )
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(experiment).write(
            to: stagingURL.appendingPathComponent("manifest.json"),
            options: .atomic
        )
        try fileManager.moveItem(at: stagingURL, to: destinationURL)
        return experiment
    }

    func list() throws -> [SavedExperiment] {
        guard fileManager.fileExists(atPath: rootURL.path) else { return [] }
        return try fileManager.contentsOfDirectory(
            at: rootURL,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        )
        .map { try manifest(at: $0) }
        .sorted {
            if $0.savedAt == $1.savedAt { return $0.id < $1.id }
            return $0.savedAt > $1.savedAt
        }
    }

    func load(id requestedID: String) throws -> (SavedExperiment, Data) {
        guard let uuid = UUID(uuidString: requestedID) else {
            throw ExperimentStoreError.invalidIdentifier
        }
        let id = uuid.uuidString.lowercased()
        let directoryURL = directory(for: id)
        guard fileManager.fileExists(atPath: directoryURL.path) else {
            throw ExperimentStoreError.notFound(requestedID)
        }
        let experiment = try manifest(at: directoryURL)
        guard experiment.id == id else {
            throw ExperimentStoreError.corrupt(id)
        }
        let campaign = try Data(
            contentsOf: directoryURL.appendingPathComponent("campaign.json")
        )
        guard sha256(campaign) == experiment.campaignSHA256 else {
            throw ExperimentStoreError.corrupt(id)
        }
        return (experiment, campaign)
    }

    static var defaultRootURL: URL {
        FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        )[0]
        .appendingPathComponent("FIPSD", isDirectory: true)
        .appendingPathComponent("Experiments", isDirectory: true)
    }

    private func directory(for id: String) -> URL {
        rootURL.appendingPathComponent(id, isDirectory: true)
    }

    private func manifest(at directoryURL: URL) throws -> SavedExperiment {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try decoder.decode(
            SavedExperiment.self,
            from: Data(
                contentsOf: directoryURL.appendingPathComponent("manifest.json")
            )
        )
    }

    private func campaignObject(_ campaign: Data) throws -> [String: Any] {
        guard let object = try JSONSerialization.jsonObject(with: campaign)
            as? [String: Any] else {
            throw ExperimentStoreError.invalidCampaign
        }
        return object
    }

    private func declaredFidelity(_ campaign: Data) throws -> JSONValue {
        try JSONDecoder().decode(JSONValue.self, from: campaign)
            .object?["fidelity"] ?? .null
    }

    private func resolvedName(
        _ requested: String?,
        campaign: [String: Any]
    ) throws -> String {
        if let requested {
            guard let name = requested.nilIfEmpty else {
                throw ExperimentStoreError.invalidName
            }
            return name
        }
        let metadata = campaign["metadata"] as? [String: Any]
        return (metadata?["name"] as? String)?.nilIfEmpty ?? "Saved experiment"
    }

    private func resolvedDescription(
        _ requested: String?,
        campaign: [String: Any]
    ) -> String? {
        if let requested { return requested.nilIfEmpty }
        let metadata = campaign["metadata"] as? [String: Any]
        return (metadata?["description"] as? String)?.nilIfEmpty
    }

    private func sha256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}

private extension String {
    var nilIfEmpty: String? {
        let value = trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }
}
