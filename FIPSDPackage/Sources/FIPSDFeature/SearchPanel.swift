import SwiftUI

struct SearchPanel: View {
    let summary: SearchSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Adversarial search & shrink").font(.headline)
            value("Evaluated cases", summary.evaluated.formatted())
            value("Best case", summary.bestCaseID)
            if let nodes = summary.minimizedNodes { value("Minimized nodes", nodes.formatted()) }
            ForEach(summary.metrics.prefix(8)) { metric in
                value(metric.name, metric.value.formatted())
            }
            if !summary.shrinkChanges.isEmpty {
                Text("Accepted reductions").font(.caption).foregroundStyle(.secondary)
                ForEach(Array(summary.shrinkChanges.prefix(8)), id: \.self) {
                    Text("• " + $0).font(.caption2)
                }
                if summary.shrinkChanges.count > 8 {
                    Text("+ \(summary.shrinkChanges.count - 8) more in the evidence bundle")
                        .font(.caption2).foregroundStyle(.secondary)
                }
            }
        }
        .accessibilityIdentifier("searchSummaryPanel")
    }

    private func value(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(value).lineLimit(1).textSelection(.enabled)
        }
        .font(.caption)
    }
}
