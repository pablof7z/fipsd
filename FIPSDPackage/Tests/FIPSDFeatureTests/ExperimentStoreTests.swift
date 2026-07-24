import Foundation
import Testing
@testable import FIPSDFeature

@Test func experimentStorePersistsExactCampaignAndProvenance() throws {
    let (store, root) = try temporaryExperimentStore()
    defer { try? FileManager.default.removeItem(at: root) }
    let campaign = try CampaignBuilder.data(for: CampaignConfiguration())

    let saved = try store.save(
        campaign: campaign,
        name: "Baseline convergence",
        description: "Reusable deterministic baseline",
        sourceRunID: "run-42",
        sourceResultFidelity: "semantic-exact"
    )
    let listed = try store.list()
    let (loaded, loadedCampaign) = try store.load(id: saved.id)

    #expect(listed == [saved])
    #expect(loaded == saved)
    #expect(loadedCampaign == campaign)
    #expect(saved.name == "Baseline convergence")
    #expect(saved.description == "Reusable deterministic baseline")
    #expect(saved.sourceRunID == "run-42")
    #expect(saved.declaredFidelity.object?["protocol"]?.string == "semantic-exact")
    #expect(saved.sourceResultFidelity == "semantic-exact")
    #expect(saved.campaignSHA256.count == 64)
}

@Test func experimentStoreDefaultsToCampaignMetadata() throws {
    let (store, root) = try temporaryExperimentStore()
    defer { try? FileManager.default.removeItem(at: root) }
    let campaign = try CampaignBuilder.data(for: CampaignConfiguration())

    let saved = try store.save(
        campaign: campaign,
        name: nil,
        description: nil,
        sourceRunID: nil,
        sourceResultFidelity: nil
    )

    #expect(saved.name == "interactive-root-wave")
    #expect(saved.description != nil)
}

@Test func experimentStoreRejectsTamperedCampaign() throws {
    let (store, root) = try temporaryExperimentStore()
    defer { try? FileManager.default.removeItem(at: root) }
    let saved = try store.save(
        campaign: try CampaignBuilder.data(for: CampaignConfiguration()),
        name: "Tamper test",
        description: nil,
        sourceRunID: nil,
        sourceResultFidelity: nil
    )
    let campaignURL = root
        .appendingPathComponent(saved.id, isDirectory: true)
        .appendingPathComponent("campaign.json")
    try Data("{}".utf8).write(to: campaignURL, options: .atomic)

    #expect(throws: ExperimentStoreError.self) {
        try store.load(id: saved.id)
    }
}

@MainActor
@Test func mcpControlSavesListsAndRerunsExactCampaign() throws {
    let (store, root) = try temporaryExperimentStore()
    defer { try? FileManager.default.removeItem(at: root) }
    let model = WorkbenchModel(experimentStore: store)
    defer {
        model.runTask?.cancel()
        model.playbackTask?.cancel()
    }
    let campaign = try CampaignBuilder.data(for: model.configuration)
    model.activeCampaign = campaign
    model.summary.runID = "source-run"
    model.summary.fidelity = "semantic-exact"

    let save = model.handleControl(AppControlRequest(
        id: "save",
        token: "",
        command: "save_experiment",
        arguments: [
            "name": .string("Saved through MCP"),
            "description": .string("Exact replay input")
        ]
    ))
    let savedID = try #require(
        save.result?.object?["experiment"]?.object?["id"]?.string
    )
    let list = model.handleControl(AppControlRequest(
        id: "list",
        token: "",
        command: "list_experiments",
        arguments: [:]
    ))
    #expect(list.result?.object?["count"]?.int == 1)

    model.activeCampaign = nil
    let rerun = model.handleControl(AppControlRequest(
        id: "rerun",
        token: "",
        command: "rerun_experiment",
        arguments: ["id": .string(savedID)]
    ))
    #expect(rerun.ok)
    #expect(model.activeCampaign == campaign)
    #expect(model.generatedSpec == String(decoding: campaign, as: UTF8.self))
}

private func temporaryExperimentStore() throws -> (ExperimentStore, URL) {
    let root = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    return (ExperimentStore(rootURL: root), root)
}
