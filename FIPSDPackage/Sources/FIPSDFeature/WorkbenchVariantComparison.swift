import Foundation

extension WorkbenchModel {
    func runVariantComparison() {
        comparisonTask?.cancel()
        comparison = nil
        comparisonStatus = "Running baseline…"
        isComparing = true
        let baselineDebounce = configuration.debounceMilliseconds
        let candidateDebounce = comparisonDebounceMilliseconds
        var baselineConfiguration = configuration
        var candidateConfiguration = configuration
        baselineConfiguration.debounceMilliseconds = baselineDebounce
        candidateConfiguration.debounceMilliseconds = candidateDebounce
        comparisonTask = Task {
            do {
                let baseline = try await runVariant(
                    configuration: baselineConfiguration, label: "baseline"
                )
                guard !Task.isCancelled else { return }
                comparisonStatus = "Running candidate…"
                let candidate = try await runVariant(
                    configuration: candidateConfiguration, label: "candidate"
                )
                guard !Task.isCancelled else { return }
                comparison = VariantComparison.compare(
                    baseline: baseline,
                    candidate: candidate,
                    baselineDebounceMS: baselineDebounce,
                    candidateDebounceMS: candidateDebounce
                )
                analysis = candidate.analysis
                if visualizationMode == .anomalies {
                    refreshRendererProjection()
                }
                comparisonStatus = "Comparison complete. Open Analysis to inspect deltas."
                isComparing = false
            } catch {
                isComparing = false
                comparisonStatus = "Comparison failed: \(error.localizedDescription)"
            }
        }
    }

    private func runVariant(
        configuration: CampaignConfiguration, label: String
    ) async throws -> VariantRunEvidence {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("fips-variant-\(UUID().uuidString)", isDirectory: true)
        let evidence = root.appendingPathComponent(label, isDirectory: true)
        try FileManager.default.createDirectory(at: evidence, withIntermediateDirectories: true)
        let campaign = root.appendingPathComponent("\(label)-campaign.json")
        try CampaignBuilder.data(for: configuration).write(to: campaign, options: .atomic)
        let client = try EngineClient.bundled()
        try await client.stream(campaignURL: campaign, evidenceURL: evidence) { _ in }
        let artifact = evidence.appendingPathComponent("artifact.json")
        let document = try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: artifact))
        guard let object = document.object, let trace = object["event_trace"]?.array else {
            throw ArtifactLoadError.missingTrace
        }
        return VariantRunEvidence(
            analysis: ArtifactAnalysis.parse(object), events: trace.compactMap(SimulationEvent.init)
        )
    }
}
