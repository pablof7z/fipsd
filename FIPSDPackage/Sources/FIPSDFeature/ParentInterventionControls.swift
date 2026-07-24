import SwiftUI

struct ParentInterventionControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Parent & ancestry") {
            VStack(alignment: .leading, spacing: 8) {
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 7) {
                    row(
                        "Cycles",
                        value: $model.configuration.parentOscillationCycles,
                        suffix: ""
                    )
                    row(
                        "Interval",
                        value: $model.configuration.parentOscillationIntervalMilliseconds,
                        suffix: "ms"
                    )
                    row(
                        "Hysteresis",
                        value: $model.configuration.parentHysteresisPercent,
                        suffix: "%"
                    )
                    row(
                        "Hold-down",
                        value: $model.configuration.parentHoldDownMilliseconds,
                        suffix: "ms"
                    )
                }
                Button("Swap parent ancestry", systemImage: "arrow.triangle.swap") {
                    model.scheduleParentIntervention("swap-parent-ancestry")
                }
                Button("Oscillate parent quality", systemImage: "waveform.path.ecg") {
                    model.scheduleParentIntervention("alternate-parent-quality")
                }
                Text(
                    "Uses the selected node when it has two converged same-root parents; "
                        + "otherwise the engine chooses the first eligible node."
                )
                .font(.caption2)
                .foregroundStyle(.secondary)
            }
            .padding(.top, 4)
        }
    }

    private func row(
        _ label: String,
        value: Binding<Int>,
        suffix: String
    ) -> some View {
        GridRow {
            Text(label).font(.caption)
            TextField(label, value: value, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 70)
            Text(suffix).font(.caption2).foregroundStyle(.secondary)
        }
    }
}
