import SwiftUI

struct SearchControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Adversarial search & shrink") {
            VStack(alignment: .leading, spacing: 9) {
                Stepper(
                    "Evaluate up to \(model.searchEvaluations) cases",
                    value: $model.searchEvaluations, in: 2...32
                )
                Button("Find and minimize worst case", systemImage: "scope") {
                    model.runAdversarialSearch()
                }
                .accessibilityIdentifier("adversarialSearchButton")
                .disabled(model.isRunning)
                Text(model.searchStatus).font(.caption2).foregroundStyle(.secondary)
                Text("Builds a bounded pairwise matrix around scale, topology, cadence, attachment, and traffic; maximizes amplification and stall, then shrinks the winner at 90% of its score.")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            .padding(.top, 6)
        }
    }
}
