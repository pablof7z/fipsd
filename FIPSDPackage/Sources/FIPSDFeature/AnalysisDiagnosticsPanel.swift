import SwiftUI

struct AnalysisDiagnosticsPanel: View {
    let diagnostics: ArtifactDiagnostics

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Recorded distributions").font(.headline)
            if diagnostics.deliverySamples > 0 {
                value("Delivery samples", diagnostics.deliverySamples.formatted())
                value("Latency p50", duration(diagnostics.latencyP50NS))
                value("Latency p95", duration(diagnostics.latencyP95NS))
                value("Latency p99", duration(diagnostics.latencyP99NS))
            } else {
                Text("No scheduled delivery samples were retained.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            if diagnostics.bloomFPRSamples > 0 {
                Text("Bloom false-positive distribution").font(.subheadline.weight(.semibold))
                value("Samples", diagnostics.bloomFPRSamples.formatted())
                value("FPR p50", rate(diagnostics.bloomFPRP50PPB))
                value("FPR p95", rate(diagnostics.bloomFPRP95PPB))
                value("FPR p99", rate(diagnostics.bloomFPRP99PPB))
            }
            Text("Bytes by protocol plane").font(.subheadline.weight(.semibold))
            bars(diagnostics.planeLoads, byteValues: true)
            Text("Queue occupancy at enqueue").font(.subheadline.weight(.semibold))
            bars(diagnostics.queueHistogram, byteValues: false)
            if !diagnostics.congestion.isEmpty {
                Text("Zone / transport congestion").font(.subheadline.weight(.semibold))
                ForEach(diagnostics.congestion.prefix(8)) { cell in
                    value("\(cell.from) → \(cell.to)", "\(bytes(cell.bytes)) · \(cell.frames) frames")
                }
            }
            Text("Percentiles and buckets use recorded due-event samples only.")
                .font(.caption2).foregroundStyle(.secondary)
        }
    }

    private func bars(_ buckets: [DiagnosticBucket], byteValues: Bool) -> some View {
        let maximum = buckets.map(\.count).max() ?? 1
        return VStack(alignment: .leading, spacing: 6) {
            ForEach(buckets) { bucket in
                VStack(alignment: .leading, spacing: 2) {
                    value(bucket.label, byteValues ? bytes(bucket.count) : bucket.count.formatted())
                    GeometryReader { geometry in
                        Capsule().fill(.quaternary).overlay(alignment: .leading) {
                            Capsule().fill(.cyan.opacity(0.65))
                                .frame(width: geometry.size.width * CGFloat(bucket.count) / CGFloat(max(maximum, 1)))
                        }
                    }.frame(height: 4)
                }
            }
        }
    }

    private func value(_ label: String, _ result: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(result).lineLimit(1).textSelection(.enabled)
        }.font(.caption)
    }

    private func duration(_ nanoseconds: UInt64) -> String {
        String(format: "%.3f ms", Double(nanoseconds) / 1e6)
    }

    private func bytes(_ count: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: count), countStyle: .binary)
    }

    private func rate(_ ppb: UInt64) -> String {
        String(format: "%.6f%%", Double(ppb) / 10_000_000)
    }
}
