import Foundation

@MainActor
extension WorkbenchModel {
    func declaredRendererFidelity(from campaign: Data) -> String? {
        guard let document = try? JSONSerialization.jsonObject(with: campaign),
              let root = document as? [String: Any],
              let fidelity = root["fidelity"] as? [String: Any] else {
            return nil
        }
        let values = [
            fidelity["scale"] as? String,
            fidelity["protocol"] as? String,
            fidelity["wire"] as? String
        ].compactMap { $0 }
        guard !values.isEmpty else { return nil }
        return "declared campaign · " + values.joined(separator: " · ")
    }

    func configureRendererEvidence(in directory: URL) throws {
        let writer = try RenderFrameLogWriter(directory: directory)
        renderFrameWriter = writer
        rendererEvidenceURL = writer.outputURL
        rendererEvidenceError = nil
        renderFrameIndex = 0
        recordedRenderFrame = nil
    }

    func configureImportedRendererEvidence() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(
                "fips-render-replay-\(UUID().uuidString)",
                isDirectory: true
            )
        try configureRendererEvidence(in: directory)
    }

    func publishRenderFrame(_ batch: DisplayProjectionBatch) {
        displayProjectionBatch = batch
        let nextFrame = RenderFrame(
            state: state,
            virtualTimeNS: virtualTimeNS,
            visualizationMode: visualizationMode,
            anomalyNodeIDs: analysis.anomalyNodeIDs,
            cohortState: cohortState,
            selectedNodeID: selectedNodeID,
            displayBatch: batch,
            sourceFidelity: summary.fidelity
        )
        selectedNodeID = nextFrame.selectedNodeID
        renderFrame = nextFrame
        guard let renderFrameWriter else { return }
        let evidence = RenderFrameEvidence(
            frameIndex: renderFrameIndex,
            frame: nextFrame,
            previous: recordedRenderFrame
        )
        do {
            try renderFrameWriter.append(evidence)
            recordedRenderFrame = nextFrame
            renderFrameIndex += 1
        } catch {
            rendererEvidenceError = error.localizedDescription
        }
    }

    func selectVisualizationMode(_ mode: VisualizationMode) {
        guard visualizationMode != mode else { return }
        visualizationMode = mode
        publishRenderFrame(.viewChange(at: virtualTimeNS))
    }

    func selectRenderedNode(_ nodeID: Int?) {
        guard selectedNodeID != nodeID else { return }
        selectedNodeID = nodeID
        publishRenderFrame(.viewChange(at: virtualTimeNS))
    }

    func refreshRendererProjection() {
        publishRenderFrame(.viewChange(at: virtualTimeNS))
    }

    func flushRendererEvidence() async throws {
        try await renderFrameWriter?.flush()
    }

    func finalizeRendererEvidence() {
        guard let renderFrameWriter else { return }
        Task { [weak self] in
            do {
                try await renderFrameWriter.flush()
            } catch {
                self?.rendererEvidenceError = error.localizedDescription
            }
        }
    }
}
