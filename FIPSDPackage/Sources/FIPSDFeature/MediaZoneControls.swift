import SwiftUI

struct MediaZoneControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Network zones & shared media") {
            VStack(alignment: .leading, spacing: 9) {
                Toggle("Share capacity inside zones", isOn: $model.configuration.mediaZonesEnabled)
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 7) {
                    row("Zones", value: $model.configuration.mediaZoneCount, suffix: "")
                    row("Capacity", value: $model.configuration.mediaZoneBandwidthMbps, suffix: "Mbit/s")
                    decimalRow("Latency", value: $model.configuration.mediaZoneLatencyMilliseconds, suffix: "ms")
                    row("Loss", value: $model.configuration.mediaZoneLossPPM, suffix: "ppm")
                    row("MTU", value: $model.configuration.mediaZoneMTUBytes, suffix: "B")
                    row("Queue", value: $model.configuration.mediaZoneQueueKB, suffix: "KiB")
                }
                .disabled(!model.configuration.mediaZonesEnabled)
                Text("Nodes are deterministically assigned round-robin. Every intra-zone edge contends for one half-duplex serialization queue; cross-zone edges retain endpoint bottlenecks.")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            .padding(.top, 6)
        }
    }

    private func row(_ label: String, value: Binding<Int>, suffix: String) -> some View {
        GridRow {
            Text(label).foregroundStyle(.secondary)
            TextField(label, value: value, format: .number)
                .textFieldStyle(.roundedBorder).frame(width: 82)
            Text(suffix).foregroundStyle(.tertiary)
        }
    }

    private func decimalRow(
        _ label: String, value: Binding<Double>, suffix: String
    ) -> some View {
        GridRow {
            Text(label).foregroundStyle(.secondary)
            TextField(label, value: value, format: .number)
                .textFieldStyle(.roundedBorder).frame(width: 82)
            Text(suffix).foregroundStyle(.tertiary)
        }
    }
}
