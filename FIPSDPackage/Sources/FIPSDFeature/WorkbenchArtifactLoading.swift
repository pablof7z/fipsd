import Foundation

extension WorkbenchModel {
    func loadArtifact(_ url: URL) {
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        do {
            try loadArtifactData(Data(contentsOf: url), sourceURL: url)
        } catch { failArtifactLoad(error) }
    }

    func loadArtifactData(_ data: Data, sourceURL: URL? = nil) throws {
        let document = try JSONDecoder().decode(JSONValue.self, from: data)
        guard let root = document.object,
              let trace = root["event_trace"]?.array else {
            throw ArtifactLoadError.missingTrace
        }
        let loaded = trace.compactMap(SimulationEvent.init)
        guard loaded.count == trace.count, !loaded.isEmpty else {
            throw ArtifactLoadError.invalidTrace
        }
        reset()
        events = loaded
        streamComplete = true
        isRunning = false
        evidenceURL = sourceURL?.deletingLastPathComponent()
        try configureImportedRendererEvidence()
        populateSummary(root)
        analysis = ArtifactAnalysis.parse(root)
        cohortState = CohortArtifactState.parse(root)
        if cohortState != nil { visualizationMode = .cohorts }
        status = "Loaded saved artifact. Playback, scrubbing, and inspection are ready."
        isPlaying = true
        startPlaybackLoop()
    }

    private func populateSummary(_ root: [String: JSONValue]) {
        guard let manifest = root["manifest"]?.object else { return }
        summary.runID = manifest["run_id"]?.string ?? ""
        summary.artifactID = manifest["artifact_id"]?.string ?? ""
        if let fidelity = manifest["fidelity"]?.object {
            summary.fidelity = [
                fidelity["scale"]?.string ?? "unknown scale",
                fidelity["protocol"]?.string ?? "unknown protocol",
                fidelity["wire"]?.string ?? "unknown wire"
            ].joined(separator: " · ")
        }
        if let report = root["reports"]?.array?.first?.object {
            summary.finalRoot = report["final_root"]?.string ?? ""
            summary.quiescenceNS = report["quiescence_ns"]?.uint64 ?? 0
        }
        let assertions = root["assertion_results"]?.array ?? []
        summary.outcome = assertions.allSatisfy {
            $0.object?["outcome"]?.string == "pass"
        } ? "pass" : "fail"
    }

    private func failArtifactLoad(_ error: Error) {
        isPlaying = false
        errorMessage = error.localizedDescription
        status = "Could not load artifact."
    }
}

enum ArtifactLoadError: LocalizedError {
    case missingTrace
    case invalidTrace

    var errorDescription: String? {
        switch self {
        case .missingTrace: "The selected JSON is not a run artifact with an event_trace."
        case .invalidTrace: "The artifact event trace is empty or contains invalid events."
        }
    }
}
