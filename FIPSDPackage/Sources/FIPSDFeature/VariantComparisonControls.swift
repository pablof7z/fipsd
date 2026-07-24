import SwiftUI

struct VariantComparisonControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Compare protocol parameters") {
            VStack(alignment: .leading, spacing: 9) {
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 7) {
                    GridRow {
                        Text("Baseline").foregroundStyle(.secondary)
                        Text(model.configuration.debounceMilliseconds.formatted())
                        Text("ms debounce").foregroundStyle(.tertiary)
                    }
                    GridRow {
                        Text("Candidate").foregroundStyle(.secondary)
                        TextField(
                            "Candidate debounce",
                            value: $model.comparisonDebounceMilliseconds,
                            format: .number.precision(.fractionLength(0...3))
                        )
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 90)
                        Text("ms debounce").foregroundStyle(.tertiary)
                    }
                }
                Button("Run side-by-side comparison", systemImage: "rectangle.split.2x1") {
                    model.runVariantComparison()
                }
                .accessibilityIdentifier("compareVariantsButton")
                .disabled(model.isComparing || model.isRunning)
                Text(model.comparisonStatus).font(.caption2).foregroundStyle(.secondary)
                Text("Both runs reuse the same seed, generated topology, traffic, and interventions; only this protocol timer changes.")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            .padding(.top, 6)
        }
    }
}
