import Foundation

extension WorkbenchModel {
    func runBillionNodeCohort() {
        runTask?.cancel()
        playbackTask?.cancel()
        reset()
        isRunning = true
        status = "Running the one-billion-node analytical cohort model…"
        runTask = Task {
            do {
                let root = FileManager.default.temporaryDirectory
                    .appendingPathComponent("fips-cohort-\(UUID().uuidString)", isDirectory: true)
                let evidence = root.appendingPathComponent("evidence", isDirectory: true)
                try FileManager.default.createDirectory(at: evidence, withIntermediateDirectories: true)
                let campaign = root.appendingPathComponent("campaign.json")
                try CampaignBuilder.cohortData(for: configuration, nodes: 1_000_000_000)
                    .write(to: campaign, options: .atomic)
                let client = try EngineClient.bundled()
                try await client.scaleRun(
                    campaignURL: campaign, evidenceURL: evidence
                )
                let sensitivity = root.appendingPathComponent("billion-sensitivity.json")
                try await client.scaleBillionDemo(campaignURL: campaign, outputURL: sensitivity)
                guard !Task.isCancelled else { return }
                let artifact = evidence.appendingPathComponent("artifact.json")
                try loadArtifactData(Data(contentsOf: artifact), sourceURL: artifact)
                let document = try JSONDecoder().decode(
                    JSONValue.self, from: Data(contentsOf: sensitivity)
                )
                scaleSensitivity = document.object.flatMap(ScaleSensitivity.parse)
                status = "Cohort run complete. Bands, bounds, sensitivity, and exact sample are ready."
                isRunning = false
                isPlaying = false
            } catch { fail(error) }
        }
    }
}
