import SwiftUI

struct TrafficControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Synthetic payload traffic") {
            VStack(alignment: .leading, spacing: 9) {
                Toggle(
                    "Route payloads through the graph",
                    isOn: $model.configuration.trafficEnabled
                )
                Picker("Pattern", selection: $model.configuration.trafficModel) {
                    Text("Uniform random").tag("uniform-random")
                    Text("One-to-one permutation").tag("permutation")
                    Text("All-to-all").tag("all-to-all")
                    Text("Hotspot / Zipf").tag("zipf")
                    Text("Many-to-one incast").tag("incast")
                    Text("One-to-many fanout").tag("outcast")
                    Text("Cross network cut").tag("cross-min-cut")
                    Text("Elephants and mice").tag("elephants-and-mice")
                    Text("Persistent segmented streams").tag("persistent-streams")
                    Text("Synchronized bursts").tag("bursty")
                    Text("Session churn").tag("session-churn")
                    Text("Payload/MTU sweep").tag("payload-sweep")
                }
                .disabled(!model.configuration.trafficEnabled)
                trafficGrid
                    .disabled(!model.configuration.trafficEnabled)
                Text(help)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding(.top, 6)
        }
    }

    private var trafficGrid: some View {
        Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 8) {
            intRow(
                model.configuration.trafficModel == "persistent-streams" ? "Streams" : "Flows",
                value: $model.configuration.trafficFlows
            )
            intRow(
                model.configuration.trafficModel == "persistent-streams" ? "Segment" : "Payload",
                value: $model.configuration.trafficPayloadBytes,
                suffix: "B"
            )
            if model.configuration.trafficModel == "persistent-streams" {
                intRow(
                    "Segments / stream",
                    value: $model.configuration.trafficSegmentsPerStream
                )
            }
            if model.configuration.trafficModel == "bursty" {
                intRow("Burst size", value: $model.configuration.trafficBurstSize)
                intRow(
                    "Burst interval",
                    value: $model.configuration.trafficBurstIntervalMilliseconds,
                    suffix: "ms"
                )
            } else {
                intRow("Rate", value: $model.configuration.trafficRateMbps, suffix: "Mbit/s")
            }
        }
    }

    private var help: String {
        switch model.configuration.trafficModel {
        case "persistent-streams":
            "Each stream reuses one session while independently animated segments share graph queues with control traffic."
        case "bursty":
            "Every burst offers several flows at the same virtual time, exposing synchronized queue pressure and loss."
        default:
            "Each payload follows a stable shortest active path and shares the same per-edge queues as control traffic."
        }
    }

    private func intRow(
        _ title: String,
        value: Binding<Int>,
        suffix: String = ""
    ) -> some View {
        GridRow {
            Text(title).foregroundStyle(.secondary)
            TextField(title, value: value, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 90)
            Text(suffix).foregroundStyle(.tertiary)
        }
    }
}
