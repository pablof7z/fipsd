import SwiftUI

struct ResourceControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Node resource limits") {
            VStack(alignment: .leading, spacing: 9) {
                Toggle(
                    "Slow root and leaf (10× CPU)",
                    isOn: $model.configuration.heterogeneousResources
                )
                Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 8) {
                    row("CPU", value: $model.configuration.cpuUnitsPerMS, suffix: "units/ms")
                    row("Memory", value: $model.configuration.memoryMB, suffix: "MiB")
                    row("Node queue", value: $model.configuration.nodeQueueKB, suffix: "KiB")
                    row("Tables", value: $model.configuration.tableEntries, suffix: "entries")
                    row("Coord cache", value: $model.configuration.coordinateCacheEntries, suffix: "entries")
                    row("Lookup TTL", value: $model.configuration.lookupTTL, suffix: "hops")
                    row("Attempts", value: $model.configuration.lookupAttempts, suffix: "")
                }
                Text("Service work competes on virtual CPU. Retained sessions, cache entries, allocations, and queues fail explicitly at configured capacity.")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            .padding(.top, 6)
        }
    }

    private func row(
        _ title: String, value: Binding<Int>, suffix: String
    ) -> some View {
        GridRow {
            Text(title).foregroundStyle(.secondary)
            TextField(title, value: value, format: .number)
                .textFieldStyle(.roundedBorder).frame(width: 82)
            Text(suffix).foregroundStyle(.tertiary)
        }
    }
}
