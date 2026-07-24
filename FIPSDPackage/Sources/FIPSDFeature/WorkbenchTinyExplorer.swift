import Foundation

extension WorkbenchModel {
    func runTinyExploration() {
        runTask?.cancel()
        isRunning = true
        tinySummary = nil
        tinyStatus = "Enumerating bounded action orders…"
        runTask = Task {
            do {
                let root = FileManager.default.temporaryDirectory
                    .appendingPathComponent("fips-tiny-\(UUID().uuidString)", isDirectory: true)
                try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
                let campaign = root.appendingPathComponent("campaign.json")
                let evidence = root.appendingPathComponent("evidence", isDirectory: true)
                try CampaignBuilder.tinyExplorationData(
                    for: configuration, maximumNodes: tinyMaximumNodes
                ).write(to: campaign, options: .atomic)
                try await EngineClient.bundled().run(arguments: [
                    "explore", "tiny", campaign.path,
                    "--maximum-nodes", String(tinyMaximumNodes),
                    "--maximum-actions", String(tinyMaximumActions),
                    "--output", evidence.path
                ])
                let data = try Data(contentsOf: evidence.appendingPathComponent("report.json"))
                let value = try JSONDecoder().decode(JSONValue.self, from: data)
                guard let object = value.object else { throw CocoaError(.fileReadCorruptFile) }
                tinySummary = TinyExplorationSummary.parse(object)
                evidenceURL = evidence
                tinyStatus = "Exhaustive tiny-state exploration complete."
                isRunning = false
            } catch {
                isRunning = false
                tinyStatus = "Tiny-state exploration failed: \(error.localizedDescription)"
            }
        }
    }
}
