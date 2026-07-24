import SwiftUI

struct VariantComparisonPanel: View {
    let comparison: VariantComparison

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Protocol parameter comparison").font(.headline)
            Text(comparison.baselineLabel).font(.caption)
            Text(comparison.candidateLabel).font(.caption)
            Label(
                comparison.compatible ? "Comparable runs" : "Informational only",
                systemImage: comparison.compatible ? "checkmark.circle" : "exclamationmark.triangle"
            )
            .font(.caption)
            Text(comparison.compatibility).font(.caption2).foregroundStyle(.secondary)
            if let divergence = comparison.firstDivergence {
                value("First divergence", divergence)
            } else {
                value("Semantic trace", "identical")
            }
            Grid(alignment: .trailing, horizontalSpacing: 8, verticalSpacing: 5) {
                GridRow {
                    Text("Metric").gridColumnAlignment(.leading)
                    Text("Base")
                    Text("Candidate")
                    Text("Δ")
                }
                .foregroundStyle(.secondary)
                ForEach(comparison.deltas.prefix(12)) { delta in
                    GridRow {
                        Text(delta.name).lineLimit(1).gridColumnAlignment(.leading)
                        Text(compact(delta.baseline))
                        Text(compact(delta.candidate))
                        Text(signed(delta.delta))
                            .foregroundStyle(delta.delta == 0 ? .secondary : .primary)
                    }
                }
            }
            .font(.caption2)
        }
        .accessibilityIdentifier("variantComparisonPanel")
    }

    private func value(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.caption2).textSelection(.enabled)
        }
    }

    private func compact(_ value: Int64) -> String {
        value.formatted(.number.notation(.compactName))
    }

    private func signed(_ value: Int64) -> String {
        value == 0 ? "0" : (value > 0 ? "+" : "") + compact(value)
    }
}
