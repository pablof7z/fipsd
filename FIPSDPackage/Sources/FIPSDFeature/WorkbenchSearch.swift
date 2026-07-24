import Foundation

extension WorkbenchModel {
    func runAdversarialSearch() {
        runTask?.cancel()
        isRunning = true
        searchSummary = nil
        searchStatus = "Planning a pairwise campaign…"
        runTask = Task {
            do {
                let root = FileManager.default.temporaryDirectory
                    .appendingPathComponent("fips-search-\(UUID().uuidString)", isDirectory: true)
                try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
                let campaign = root.appendingPathComponent("campaign.json")
                let plan = root.appendingPathComponent("plan.json")
                let search = root.appendingPathComponent("search.json")
                let reproduction = root.appendingPathComponent("best-reproduction.json")
                let shrink = root.appendingPathComponent("shrink.json")
                try CampaignBuilder.searchData(for: configuration).write(to: campaign, options: .atomic)
                let client = try EngineClient.bundled()
                try await client.run(arguments: [
                    "campaign", "plan", campaign.path, "--mode", "covering",
                    "--strength", "2", "--output", plan.path
                ])
                searchStatus = "Searching bounded cases…"
                try await client.run(arguments: [
                    "campaign", "search", plan.path,
                    "--maximum-evaluations", String(searchEvaluations), "--output", search.path
                ])
                let searchDocument = try decodeObject(search)
                let best = try bestCandidate(searchDocument)
                try JSONEncoder.pretty.encode(best.reproduction).write(to: reproduction, options: .atomic)
                searchStatus = "Shrinking the highest-amplification case…"
                try await client.run(arguments: [
                    "campaign", "shrink", reproduction.path,
                    "--metric", "amplification-ppm",
                    "--minimum", String(best.amplification * 9 / 10),
                    "--workers", "2", "--output", shrink.path
                ])
                let shrinkDocument = try decodeObject(shrink)
                searchSummary = SearchSummary.parse(search: searchDocument, shrink: shrinkDocument)
                if let artifact = best.artifact.object { analysis = ArtifactAnalysis.parse(artifact) }
                evidenceURL = root
                searchStatus = "Search and automatic minimization complete."
                isRunning = false
            } catch {
                isRunning = false
                searchStatus = "Search failed: \(error.localizedDescription)"
            }
        }
    }

    private func decodeObject(_ url: URL) throws -> [String: JSONValue] {
        let value = try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: url))
        guard let object = value.object else { throw CocoaError(.fileReadCorruptFile) }
        return object
    }

    private func bestCandidate(
        _ search: [String: JSONValue]
    ) throws -> (artifact: JSONValue, reproduction: JSONValue, amplification: UInt64) {
        guard let best = search["best_case_ids"]?.array?.first?.string,
              let candidate = search["checkpoint"]?.object?["evaluated"]?.object?[best]?.object,
              let artifact = candidate["artifact"], let reproduction = candidate["reproduction"],
              let amplification = candidate["metrics"]?.object?["amplification-ppm"]?.uint64
        else { throw CocoaError(.fileReadCorruptFile) }
        return (artifact, reproduction, amplification)
    }
}

private extension JSONEncoder {
    static var pretty: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return encoder
    }
}
