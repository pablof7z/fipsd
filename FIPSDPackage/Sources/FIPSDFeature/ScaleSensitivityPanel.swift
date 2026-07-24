import SwiftUI

struct ScaleSensitivityPanel: View {
    let sensitivity: ScaleSensitivity

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text("Billion-node sensitivity sweep").font(.headline)
            value("Scenario matrix", "\(sensitivity.scenarios.count) cases")
            value("Control-byte span", String(format: "%.2f×", sensitivity.controlSpan))
            value("Exact anomaly sample", "\(sensitivity.exactSampleNodes) nodes")
            value("Maximum cohorts", sensitivity.maximumCohorts.formatted())
            Text(sensitivity.claim).font(.caption2).foregroundStyle(.secondary)
            Text(sensitivity.warning).font(.caption2).foregroundStyle(.orange)
            ForEach(grouped, id: \.key) { group in
                VStack(alignment: .leading, spacing: 4) {
                    Text(group.key).font(.caption).foregroundStyle(.secondary)
                    ForEach(group.values) { scenario in scenarioRow(scenario) }
                }
            }
            Text("Intervals are deterministic model bounds. This 18-point sweep does not infer an unobserved phase transition or confidence distribution.")
                .font(.caption2).foregroundStyle(.secondary)
        }
        .accessibilityIdentifier("scaleSensitivityPanel")
    }

    private var grouped: [(key: String, values: [ScaleScenario])] {
        let groups = Dictionary(grouping: sensitivity.scenarios) { scenario in
            "\(scenario.topology) · \(cadence(scenario.cadenceNS)) cadence"
        }
        return groups.keys.sorted().map { key in
            (key, groups[key]!.sorted { $0.shortVariant < $1.shortVariant })
        }
    }

    private func scenarioRow(_ scenario: ScaleScenario) -> some View {
        let maximum = sensitivity.scenarios.map(\.controlBytes.numericValue).max() ?? 1
        let ratio = scenario.controlBytes.numericValue / max(1, maximum)
        return VStack(alignment: .leading, spacing: 2) {
            value(scenario.shortVariant, formatBytes(scenario.controlBytes.value))
            GeometryReader { geometry in
                Capsule().fill(.quaternary).overlay(alignment: .leading) {
                    Capsule().fill(color(scenario.shortVariant).opacity(0.7))
                        .frame(width: geometry.size.width * ratio)
                }
            }
            .frame(height: 4)
            Text("\(formatBytes(scenario.controlBytes.lower))…\(formatBytes(scenario.controlBytes.upper)) · peak queue \(formatBytes(scenario.peakQueueBytes.value))")
                .font(.caption2).foregroundStyle(.secondary)
        }
    }

    private func value(_ label: String, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(text).lineLimit(1).textSelection(.enabled)
        }
        .font(.caption)
    }

    private func cadence(_ value: UInt64) -> String {
        String(format: "%.2f s", Double(value) / 1e9)
    }

    private func formatBytes(_ value: String) -> String {
        guard var number = Double(value) else { return value + " B" }
        let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"]
        var index = 0
        while number >= 1_000, index < units.count - 1 {
            number /= 1_000
            index += 1
        }
        return String(format: "%.1f %@", number, units[index])
    }

    private func color(_ variant: String) -> Color {
        switch variant {
        case "dampening": .green
        case "Bloom delta": .cyan
        default: .orange
        }
    }
}
