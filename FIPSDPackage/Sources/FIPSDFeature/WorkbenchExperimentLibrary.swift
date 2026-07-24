import Foundation

extension WorkbenchModel {
    func saveControlExperiment(
        _ arguments: [String: JSONValue]
    ) throws -> JSONValue {
        guard let activeCampaign else { throw AppControlError.noActiveCampaign }
        let saved = try experimentStore.save(
            campaign: activeCampaign,
            name: arguments["name"]?.string,
            description: arguments["description"]?.string,
            sourceRunID: summary.runID,
            sourceResultFidelity: summary.fidelity
        )
        return .object([
            "experiment": saved.controlValue,
            "saved_experiment_count": .integer(
                Int64(try experimentStore.list().count)
            )
        ])
    }

    func listControlExperiments() throws -> JSONValue {
        let experiments = try experimentStore.list()
        return .object([
            "experiments": .array(experiments.map(\.controlValue)),
            "count": .integer(Int64(experiments.count))
        ])
    }

    func rerunControlExperiment(
        _ arguments: [String: JSONValue]
    ) throws -> JSONValue {
        guard let id = arguments["id"]?.string, !id.isEmpty else {
            throw AppControlError.invalidArgument("id")
        }
        let (saved, campaign) = try experimentStore.load(id: id)
        generatedSpec = String(decoding: campaign, as: UTF8.self)
        startRun(
            campaign: campaign,
            author: "saved-experiment",
            authoringPrompt: "Rerun saved experiment \(saved.name)"
        )
        return .object([
            "experiment": saved.controlValue,
            "state": controlSnapshot(limit: 20)
        ])
    }
}
