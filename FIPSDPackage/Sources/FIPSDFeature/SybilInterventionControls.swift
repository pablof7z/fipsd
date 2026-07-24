import SwiftUI

struct SybilInterventionControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Authenticated Sybils") {
            VStack(alignment: .leading, spacing: 8) {
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 7) {
                    row("Identities", value: $model.configuration.sybilCount, suffix: "")
                    row(
                        "Cadence",
                        value: $model.configuration.sybilIntervalMilliseconds,
                        suffix: "ms"
                    )
                }
                Picker("Attachment", selection: $model.configuration.sybilAttachment) {
                    Text("Hub").tag("hub")
                    Text("Current root").tag("current-root")
                    Text("Leaf").tag("leaf")
                    Text("Random").tag("random")
                }
                Toggle(
                    "Grind lower-root identities",
                    isOn: $model.configuration.sybilRootGrinding
                )
                Button("Attach authenticated Sybils", systemImage: "person.3.sequence.fill") {
                    model.scheduleAuthenticatedSybils()
                }
                Text(
                    "Each identity is a real simulated node with a visible join, "
                        + "attachment edge, Tree/Bloom consequences, and attacker debit."
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
