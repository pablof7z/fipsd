import SwiftUI

struct AnalysisPanel: View {
    let analysis: ArtifactAnalysis
    let comparison: VariantComparison?
    let searchSummary: SearchSummary?
    let scaleSensitivity: ScaleSensitivity?
    let tinySummary: TinyExplorationSummary?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if hasArtifactAnalysis { fidelity }
            if let searchSummary { Divider(); SearchPanel(summary: searchSummary) }
            if let scaleSensitivity { Divider(); ScaleSensitivityPanel(sensitivity: scaleSensitivity) }
            if let tinySummary { Divider(); TinyExplorationPanel(summary: tinySummary) }
            if let comparison { Divider(); VariantComparisonPanel(comparison: comparison) }
            if hasArtifactAnalysis {
                Divider()
                causalAccounting
                if !analysis.causalFlames.isEmpty {
                    Divider()
                    CausalFlamePanel(flames: analysis.causalFlames)
                }
                Divider()
                AnalysisDiagnosticsPanel(diagnostics: analysis.diagnostics)
                Divider()
                bottlenecks
                if !analysis.rootImpacts.isEmpty { Divider(); rootWave }
                if !analysis.metrics.isEmpty { Divider(); reportedMetrics }
            }
        }
        .accessibilityIdentifier("artifactAnalysisPanel")
    }

    private var hasArtifactAnalysis: Bool {
        analysis.eventCount > 0 || analysis.representedNodes > 0
    }

    private var fidelity: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Analysis evidence").font(.headline)
            value("Represented nodes", analysis.representedNodes.formatted())
            value("Recorded events", analysis.eventCount.formatted())
            Text(analysis.fidelity).font(.caption2).foregroundStyle(.secondary)
            Text("Rankings use recorded due events; absent work is not inferred.")
                .font(.caption2).foregroundStyle(.secondary)
        }
    }

    private var causalAccounting: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Causal work by stage").font(.headline)
            value("Ledger entries", analysis.ledgerEntries.formatted())
            value("Longest event chain", analysis.longestCausalChain.formatted())
            ForEach(analysis.stages.prefix(10)) { stage in
                barRow(stage.stage, value: stage.count, maximum: analysis.stages.first?.count ?? 1)
            }
        }
    }

    private var bottlenecks: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Heavy-hitter directed links").font(.headline)
            if analysis.topEdges.isEmpty {
                Text("No queued link transmissions were recorded.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            ForEach(analysis.topEdges.prefix(8)) { edge in
                VStack(alignment: .leading, spacing: 2) {
                    value("#\(edge.from) → #\(edge.to)", bytes(edge.bytes))
                    Text("\(edge.frames) frames · peak queue \(bytes(edge.peakQueueBytes))")
                        .font(.caption2).foregroundStyle(.secondary)
                }
            }
        }
    }

    private var rootWave: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Root-arrival consequences").font(.headline)
            ForEach(analysis.rootImpacts) { impact in
                VStack(alignment: .leading, spacing: 2) {
                    value(String(impact.root.prefix(10)) + "…", impact.consequences.formatted())
                    Text("at \(duration(impact.arrivalNS)) · transitive recorded descendants")
                        .font(.caption2).foregroundStyle(.secondary)
                }
            }
        }
    }

    private var reportedMetrics: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Artifact metrics").font(.headline)
            ForEach(analysis.metrics) { metric in
                value(metric.name, metric.value + (metric.unit.isEmpty ? "" : " \(metric.unit)"))
            }
        }
    }

    private func barRow(_ label: String, value: UInt64, maximum: UInt64) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            self.value(label, value.formatted())
            GeometryReader { geometry in
                Capsule().fill(.quaternary)
                    .overlay(alignment: .leading) {
                        Capsule().fill(Color.accentColor.opacity(0.65))
                            .frame(width: geometry.size.width * CGFloat(value) / CGFloat(max(maximum, 1)))
                    }
            }
            .frame(height: 4)
        }
    }

    private func value(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(value).lineLimit(1).textSelection(.enabled)
        }
        .font(.caption)
    }

    private func duration(_ nanoseconds: UInt64) -> String {
        String(format: "%.3f s", Double(nanoseconds) / 1e9)
    }

    private func bytes(_ count: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: count), countStyle: .binary)
    }
}
