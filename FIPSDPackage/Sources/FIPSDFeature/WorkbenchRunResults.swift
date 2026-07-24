import Foundation

extension WorkbenchModel {
    func parseSummary(_ envelope: StreamEnvelope) {
        guard let payload = envelope.payload.object else { return }
        summary.runID = envelope.runID ?? ""
        summary.artifactID = payload["artifact_id"]?.string ?? ""
        summary.outcome = payload["outcome"]?.string ?? ""
        if let fidelity = payload["fidelity"]?.object {
            let protocolMode = fidelity["protocol"]?.string ?? "unknown protocol"
            let wire = fidelity["wire"]?.string ?? "unknown wire"
            let scale = fidelity["scale"]?.string ?? "unknown scale"
            let approximations = fidelity["approximations"]?.array?.count ?? 0
            summary.fidelity =
                "\(scale) · \(protocolMode) · \(wire) · \(approximations) approximation labels"
        }
        if let report = payload["report"]?.object {
            summary.finalRoot = report["final_root"]?.string ?? ""
            summary.quiescenceNS = report["quiescence_ns"]?.uint64 ?? 0
        }
    }

    func loadEvidenceAnalysis() {
        guard let url = evidenceURL?.appendingPathComponent("artifact.json"),
              let data = try? Data(contentsOf: url),
              let document = try? JSONDecoder().decode(JSONValue.self, from: data),
              let root = document.object else { return }
        analysis = ArtifactAnalysis.parse(root)
    }
}
