import Foundation
import Observation

@MainActor
@Observable
final class WorkbenchModel {
    var configuration = CampaignConfiguration()
    var provider = AuthoringProvider.automatic
    var prompt = "Show three nodes in a chain. The last node downloads a 500 MB file from the first node through the middle node, and visualize the transfer."
    var generatedSpec = ""
    var state = SimulationState()
    var events: [SimulationEvent] = []
    var cursor = 0
    var virtualTimeNS: UInt64 = 0
    var displayProjectionBatch = DisplayProjectionBatch.empty
    var renderFrame = RenderFrame(state: SimulationState(), virtualTimeNS: 0)
    var rendererEvidenceURL: URL?
    var rendererEvidenceError: String?
    var isPlaying = false
    var isRunning = false
    var speed = 1.0
    var visualizationMode = VisualizationMode.rootAdoption
    var status = "Configure an experiment, then run it."
    var errorMessage: String?
    var selectedNodeID: Int?
    var interventionEdgeID = 0
    var interventionBandwidthMbps = 1
    var interventionLatencyMilliseconds = 200.0
    var interventionJitterMilliseconds = 50.0
    var interventionLossPPM = 0
    var interventionMTUBytes = 1_280
    var explicitEdgeFrom = 0
    var explicitEdgeTo = 1
    var comparisonDebounceMilliseconds = 2_000.0
    var comparisonStatus = "Ready to compare the current scenario."
    var comparison: VariantComparison?
    var isComparing = false
    var summary = RunSummary()
    var analysis = ArtifactAnalysis()
    var cohortState: CohortArtifactState?
    var scaleSensitivity: ScaleSensitivity?
    var searchEvaluations = 6
    var searchStatus = "Ready to search a bounded scenario matrix."
    var searchSummary: SearchSummary?
    var tinyStatus = "Ready to enumerate bounded action orders."
    var tinyMaximumNodes = 6
    var tinyMaximumActions = 6
    var tinySummary: TinyExplorationSummary?
    var evidenceURL: URL?
    var activeCampaign: Data?

    var runTask: Task<Void, Never>?
    var playbackTask: Task<Void, Never>?
    var comparisonTask: Task<Void, Never>?
    var streamComplete = false
    @ObservationIgnored var controlServer: AppControlServer?
    @ObservationIgnored var renderFrameWriter: RenderFrameLogWriter?
    @ObservationIgnored var renderFrameIndex = 0
    @ObservationIgnored var recordedRenderFrame: RenderFrame?
    @ObservationIgnored let experimentStore: ExperimentStore

    init(experimentStore: ExperimentStore = ExperimentStore()) {
        self.experimentStore = experimentStore
    }

    func runConfigured() {
        do {
            let campaign = try annotated(
                try CampaignBuilder.data(for: configuration),
                author: "manual"
            )
            startRun(campaign: campaign, author: "manual", authoringPrompt: nil)
        }
        catch { fail(error) }
    }

    func generateAndRun() {
        guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        runTask?.cancel()
        isRunning = true
        errorMessage = nil
        status = "Asking local \(provider.rawValue) to author a campaign…"
        let template: Data
        do { template = try CampaignBuilder.data(for: configuration) }
        catch { fail(error); return }
        let request = prompt
        let selectedProvider = provider
        runTask = Task {
            do {
                let (rawCampaign, actualProvider) = try await PromptAuthor().generate(
                    prompt: request,
                    provider: selectedProvider,
                    template: template
                )
                guard !Task.isCancelled else { return }
                let campaign = try annotated(rawCampaign, author: actualProvider.rawValue)
                generatedSpec = String(decoding: campaign, as: UTF8.self)
                startRun(
                    campaign: campaign,
                    author: actualProvider.rawValue,
                    authoringPrompt: request
                )
            } catch { fail(error) }
        }
    }

    func loadTenThousandPreset() {
        configuration.nodes = 10_000
        configuration.arrivals = 8
        configuration.intervalSeconds = 1
        configuration.topology = "random-regular"
        configuration.averageDegree = 4
        configuration.attachment = "random"
        configuration.latencyMilliseconds = 20
        selectVisualizationMode(.cohorts)
        status = "Loaded the 10K descending-root wave preset."
    }

    func startRun(
        campaign: Data,
        author: String,
        authoringPrompt: String?,
        resumeAtNS: UInt64? = nil,
        authoringContext: Data? = nil
    ) {
        runTask?.cancel()
        playbackTask?.cancel()
        reset()
        if let declared = declaredRendererFidelity(from: campaign) {
            summary.fidelity = declared
        }
        activeCampaign = campaign
        if let resumeAtNS { virtualTimeNS = resumeAtNS }
        do {
            let runID = UUID().uuidString
            let directory = FileManager.default.temporaryDirectory
                .appendingPathComponent("fips-wind-tunnel-\(runID)", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let campaignURL = directory.appendingPathComponent("campaign.json")
            try campaign.write(to: campaignURL, options: .atomic)
            evidenceURL = directory.appendingPathComponent("evidence", isDirectory: true)
            try FileManager.default.createDirectory(
                at: evidenceURL!,
                withIntermediateDirectories: true
            )
            try configureRendererEvidence(in: evidenceURL!)
            var authoring: [String: Any] = [
                "provider": author,
                "campaign": try JSONSerialization.jsonObject(with: campaign)
            ]
            if let authoringPrompt { authoring["prompt"] = authoringPrompt }
            if let authoringContext {
                authoring["rendered_state_context"] =
                    try JSONSerialization.jsonObject(with: authoringContext)
            }
            let authoringData = try JSONSerialization.data(
                withJSONObject: authoring,
                options: [.prettyPrinted, .sortedKeys]
            )
            try authoringData.write(
                to: evidenceURL!.appendingPathComponent("authoring.json"),
                options: .atomic
            )
            let client = try EngineClient.bundled()
            isRunning = true
            isPlaying = true
            status = "Validating and running the \(author)-authored campaign…"
            startPlaybackLoop()
            runTask = Task {
                do {
                    try await client.stream(
                        campaignURL: campaignURL,
                        evidenceURL: evidenceURL!
                    ) { [weak self] envelopes in self?.receive(envelopes) }
                } catch {
                    if !Task.isCancelled { fail(error) }
                }
            }
        } catch { fail(error) }
    }

    private func receive(_ envelopes: [StreamEnvelope]) {
        var newEvents: [SimulationEvent] = []
        for envelope in envelopes {
            guard envelope.apiVersion == "experiments.fips.network/event-stream/v1alpha1" else {
                fail(EngineClientError.invalidLine("unsupported stream version \(envelope.apiVersion)"))
                return
            }
            if envelope.kind == "event", let event = SimulationEvent(envelope.payload) {
                newEvents.append(event)
            } else if envelope.kind == "stream-complete" {
                streamComplete = true
                isRunning = false
                parseSummary(envelope)
                status = summary.outcome == "pass"
                    ? "Run complete. Playback and evidence are ready."
                    : "Run complete with failed assertions. Inspect and replay the evidence."
                loadEvidenceAnalysis()
            }
        }
        events.append(contentsOf: newEvents)
    }

    func apply(_ event: SimulationEvent) {
        virtualTimeNS = max(virtualTimeNS, event.timeNS)
        state.apply(event)
        if event.kind == "input.initial-topology", state.nodes.count > 500 {
            visualizationMode = .cohorts
        }
    }

    func reset() {
        state = SimulationState()
        events = []
        cursor = 0
        virtualTimeNS = 0
        selectedNodeID = nil
        displayProjectionBatch = .empty
        renderFrame = RenderFrame(
            state: state,
            virtualTimeNS: 0,
            visualizationMode: visualizationMode
        )
        renderFrameWriter = nil
        renderFrameIndex = 0
        recordedRenderFrame = nil
        rendererEvidenceURL = nil
        rendererEvidenceError = nil
        streamComplete = false
        summary = RunSummary()
        analysis = ArtifactAnalysis()
        cohortState = nil
        scaleSensitivity = nil
        errorMessage = nil
    }

    func annotated(_ data: Data, author: String) throws -> Data {
        guard var campaign = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              var metadata = campaign["metadata"] as? [String: Any] else { return data }
        var labels = metadata["labels"] as? [String: String] ?? [:]
        labels["authoring.provider"] = author.lowercased()
        metadata["labels"] = labels
        campaign["metadata"] = metadata
        return try JSONSerialization.data(
            withJSONObject: campaign,
            options: [.prettyPrinted, .sortedKeys]
        )
    }

    func fail(_ error: Error) {
        isRunning = false
        isPlaying = false
        errorMessage = error.localizedDescription
        status = "Experiment stopped."
    }

}
