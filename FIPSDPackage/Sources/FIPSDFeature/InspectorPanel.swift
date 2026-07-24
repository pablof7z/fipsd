import SwiftUI

struct InspectorPanel: View {
    @Bindable var model: WorkbenchModel
    @State private var mode = InspectorMode.live

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                runStatus
                Divider()
                Picker("Inspector", selection: $mode) {
                    ForEach(InspectorMode.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("inspectorModePicker")
                if mode == .analysis {
                    AnalysisPanel(
                        analysis: model.analysis, comparison: model.comparison,
                        searchSummary: model.searchSummary,
                        scaleSensitivity: model.scaleSensitivity,
                        tinySummary: model.tinySummary
                    )
                } else {
                    metrics
                    Divider()
                    nodeInspector
                    Divider()
                    eventInspector
                }
            }
            .padding(16)
        }
        .frame(minWidth: 250, idealWidth: 290, maxWidth: 330)
        .background(.thinMaterial)
        .accessibilityIdentifier("rightInspector")
    }

    private var runStatus: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Run", systemImage: model.isRunning ? "waveform" : "checkmark.circle")
                .font(.headline)
            Text(model.status).font(.caption).foregroundStyle(.secondary)
            if let error = model.errorMessage {
                Text(error).font(.caption).foregroundStyle(.red).textSelection(.enabled)
            }
            if !model.summary.runID.isEmpty {
                value("Run ID", model.summary.runID)
                value("Outcome", model.summary.outcome)
                value("Final root", String(model.summary.finalRoot.prefix(16)) + "…")
                value("Quiescence", duration(model.summary.quiescenceNS))
            }
            if let url = model.evidenceURL {
                Button("Reveal evidence", systemImage: "folder") {
                    NSWorkspace.shared.activateFileViewerSelecting([url])
                }
                .controlSize(.small)
            }
        }
    }

    private var metrics: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Live accounting")
                .font(.headline)
                .accessibilityIdentifier("liveAccountingHeading")
            value("Events", model.cursor.formatted() + " / " + model.events.count.formatted())
            value("Active nodes", model.state.nodes.values.filter(\.active).count.formatted())
            value("Frames in flight", model.state.transmissions.count.formatted())
            value("Frames delivered", model.state.deliveredFrames.formatted())
            value("Control rejected", model.state.controlRejected.formatted())
            value("Bloom delivered", model.state.bloomDelivered.formatted())
            value("Bloom rejected", model.state.bloomRejected.formatted())
            value("Lookup delivered", model.state.lookupDelivered.formatted())
            value("Lookup rejected", model.state.lookupRejected.formatted())
            value("Session delivered", model.state.sessionDelivered.formatted())
            value("Session rejected", model.state.sessionRejected.formatted())
            value("Rekeys completed", model.state.rekeysCompleted.formatted())
            value("Lookup waves", model.state.lookupWaves.formatted())
            value("Parent quality pulses", model.state.parentQualityPulses.formatted())
            value(
                "Authenticated Sybils",
                model.state.authenticatedSybilArrivals.formatted()
            )
            value(
                "Parent switches suppressed",
                model.state.parentSwitchesSuppressed.formatted()
            )
            value(
                "Cache invalidations",
                model.state.coordinateCacheInvalidations.formatted()
            )
            value(
                "Failed transport classes",
                model.state.failedTransportClasses.count.formatted()
            )
            value("Coordinate cache hits", model.state.cacheHits.formatted())
            value("Payload flows", model.state.flowsDelivered.formatted())
            value("Payload rejected", model.state.flowsRejected.formatted())
            value("Object transfers", model.state.applicationTransfers.count.formatted())
            if let transfer = model.state.applicationTransfers.values.sorted(by: {
                $0.id < $1.id
            }).first {
                value(
                    "Transfer progress",
                    transfer.progress.formatted(.percent.precision(.fractionLength(1)))
                )
            }
            value("Useful delivered", ByteCountFormatter.string(fromByteCount: Int64(model.state.usefulBytesDelivered), countStyle: .binary))
            value("Transmitted", ByteCountFormatter.string(fromByteCount: Int64(model.state.transmittedBytes), countStyle: .binary))
            value("Peak link queue", ByteCountFormatter.string(fromByteCount: Int64(model.state.queuePeakBytes), countStyle: .binary))
        }
    }

    @ViewBuilder
    private var nodeInspector: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Node inspector").font(.headline)
            if let node = model.selectedNode {
                value("Node", "#\(node.id)")
                value("Status", node.active ? "active" : "offline")
                value("Root", "#\(node.root)")
                value("Parent", node.parent.map { "#\($0)" } ?? "none")
                value("Address", String(node.address.prefix(16)) + "…")
                value("Connectivity", node.transportProfile)
                value("Network zone", node.mediaZone ?? "none")
                value("Access bandwidth", bandwidth(node.bandwidthBPS))
                value("Access latency", latency(node.latencyNS))
                value("Access jitter", latency(node.jitterNS))
                value("Access MTU", "\(node.mtuBytes) B")
                if let capacity = model.selectedSharedMediumCapacityBPS {
                    value("Shared medium capacity", bandwidth(capacity))
                }
                if let bottleneck = model.selectedEffectiveBandwidthBPS {
                    value("Incident bottleneck", bandwidth(bottleneck))
                }
                value(
                    "Incident edges",
                    model.selectedIncidentEdges.prefix(8).map(String.init).joined(separator: ", ")
                )
                HStack {
                    Button("Fail", systemImage: "bolt.slash") { model.scheduleSelectedNode(active: false) }
                        .disabled(!node.active)
                    Button("Recover", systemImage: "arrow.clockwise") { model.scheduleSelectedNode(active: true) }
                        .disabled(node.active)
                }
                .controlSize(.small)
                Button("Target first edge in controls", systemImage: "scope") {
                    model.targetFirstIncidentEdge()
                }
                .controlSize(.small)
                .disabled(model.selectedIncidentEdges.isEmpty)
                if let edge = model.selectedIncidentEdge {
                    Divider()
                    Text("Incident link").font(.subheadline.weight(.semibold))
                    value("Edge", "#\(edge.id) · #\(edge.from) ↔ #\(edge.to)")
                    value("Status", edge.active ? "active" : "partitioned")
                    value("Effective bandwidth", bandwidth(edge.bandwidthBPS))
                    value("Latency", latency(edge.latencyNS))
                    value("Jitter", latency(edge.jitterNS))
                    value("Loss", "\(edge.lossPPM.formatted()) ppm")
                    value("MTU", "\(edge.mtuBytes) B")
                    value("Queue budget", bytes(edge.queueBytes))
                    value("Shared group", edge.sharedMediumGroup.map(String.init) ?? "none")
                }
                HStack {
                    Button("Isolate", systemImage: "network.slash") {
                        model.scheduleSelectedPartition(merge: false)
                    }
                    .disabled(!node.active)
                    Button("Merge", systemImage: "point.3.connected.trianglepath.dotted") {
                        model.scheduleSelectedPartition(merge: true)
                    }
                    .disabled(!node.active)
                }
                .controlSize(.small)
                Text("Schedules the input after the current cursor and starts a new deterministic run.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            } else {
                Text("Click a node to inspect or schedule a failure.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var eventInspector: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Causal event").font(.headline)
            if let event = model.state.lastEvent {
                value("Kind", event.kind)
                value("Event", event.id)
                value("At", duration(event.timeNS))
                value("Parent", event.causalParent ?? "input")
                Text(JSONValue.object(event.data).prettyDescription)
                    .font(.system(.caption2, design: .monospaced))
                    .textSelection(.enabled)
            } else {
                Text("No event at this cursor.").font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func value(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(value).lineLimit(1).textSelection(.enabled)
        }
        .font(.caption)
    }

    private func duration(_ nanoseconds: UInt64) -> String {
        String(format: "%.3f s", Double(nanoseconds) / 1e9)
    }

    private func bandwidth(_ bps: Int) -> String {
        if bps >= 1_000_000_000 { return String(format: "%.1f Gbit/s", Double(bps) / 1e9) }
        if bps >= 1_000_000 { return String(format: "%.1f Mbit/s", Double(bps) / 1e6) }
        return "\(bps.formatted()) bit/s"
    }

    private func latency(_ nanoseconds: UInt64) -> String {
        String(format: "%.1f ms", Double(nanoseconds) / 1e6)
    }

    private func bytes(_ count: Int) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(count), countStyle: .binary)
    }
}

private enum InspectorMode: String, CaseIterable, Identifiable {
    case live = "Live"
    case analysis = "Analysis"
    var id: Self { self }
}
