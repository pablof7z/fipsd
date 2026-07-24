import SwiftUI

struct TinyExplorationPanel: View {
    let summary: TinyExplorationSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Exhaustive tiny-state exploration").font(.headline)
            value("Action templates", summary.actionCount.formatted())
            value("Orders explored", "\(summary.explored.formatted()) / \(summary.expected.formatted())")
            value("Coverage", summary.exhaustive ? "exhaustive" : "incomplete")
            value("Distinct terminal states", summary.terminalStates.formatted())
            value("Counterexamples", summary.counterexamples.count.formatted())
            Text(summary.fidelity).font(.caption2).foregroundStyle(.secondary)
            ForEach(summary.counterexamples.prefix(8)) { counterexample in
                VStack(alignment: .leading, spacing: 2) {
                    Text(counterexample.order.joined(separator: " → "))
                        .font(.caption.weight(.semibold))
                    Text(counterexample.failure)
                        .font(.caption2).foregroundStyle(.red).textSelection(.enabled)
                }
            }
        }
        .accessibilityIdentifier("tinyExplorationPanel")
    }

    private func value(_ label: String, _ result: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(result).lineLimit(1).textSelection(.enabled)
        }.font(.caption)
    }
}
