import SwiftUI

struct NetworkCanvas: View {
    let state: SimulationState
    let virtualTimeNS: UInt64
    @Binding var selection: Int?
    @Binding var mode: VisualizationMode
    let cohortState: CohortArtifactState?
    let anomalyNodeIDs: Set<Int>

    var body: some View {
        GeometryReader { geometry in
            let positions = positions(in: geometry.size)
            let cohorts = CohortLayout(state: state, size: geometry.size)
            InteractiveCanvasViewport {
                Canvas { context, _ in
                    if mode == .cohorts {
                        if let cohortState {
                            CohortArtifactCanvas(
                                state: cohortState,
                                size: geometry.size
                            ).draw(context: &context)
                        } else {
                            cohorts.draw(
                                context: &context,
                                state: state,
                                virtualTimeNS: virtualTimeNS
                            )
                        }
                    } else {
                        drawEdges(context: &context, positions: positions)
                        drawTransmissions(context: &context, positions: positions)
                        drawNodes(context: &context, positions: positions)
                    }
                }
                .contentShape(Rectangle())
                .gesture(SpatialTapGesture().onEnded { value in
                    selection = mode == .cohorts
                        ? cohorts.nearestRepresentative(to: value.location)
                        : nearestNode(to: value.location, positions: positions)
                })
            }
            .overlay(alignment: .topLeading) { legend }
            .overlay(alignment: .bottomLeading) { transferProgress }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    @ViewBuilder
    private var transferProgress: some View {
        if !state.applicationTransfers.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(state.applicationTransfers.values.sorted { $0.id < $1.id }.prefix(3)) {
                    transfer in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Label(transfer.id, systemImage: "arrow.down.doc.fill")
                            Spacer()
                            Text(transfer.progress, format: .percent.precision(.fractionLength(1)))
                        }
                        ProgressView(value: transfer.progress)
                            .tint(.yellow)
                        Text("\(transfer.routeLabel) · "
                            + "\(bytes(transfer.deliveredBytes)) / \(bytes(transfer.totalBytes))")
                        .foregroundStyle(.secondary)
                    }
                }
            }
            .font(.caption)
            .padding(10)
            .frame(width: 310)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
            .padding(12)
        }
    }

    private func bytes(_ value: Int) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(value), countStyle: .file)
    }

    private var legend: some View {
        HStack(spacing: 14) {
            Picker("View", selection: $mode) {
                ForEach(VisualizationMode.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .frame(width: 150).accessibilityIdentifier("visualizationModePicker")
            if mode == .cohorts {
                Text(cohortState?.fidelity ?? "transport × root × depth cohorts")
                    .foregroundStyle(.secondary)
            } else if mode == .connectivity {
                transportLegend("Wi-Fi", color: .cyan)
                transportLegend("BLE", color: .blue)
                transportLegend("Tor", color: .purple)
                transportLegend("Ethernet", color: .green)
            } else if mode == .sharedMedium {
                Text("color = zone · shared queue edges are emphasized")
                    .foregroundStyle(.secondary)
            } else if mode == .anomalies {
                Text("sample = endpoints of the 12 heaviest recorded directed links")
                    .foregroundStyle(.secondary)
            } else {
                Label("control frame", systemImage: "arrow.right").foregroundStyle(.pink)
                Label("Bloom", systemImage: "arrow.right").foregroundStyle(.cyan)
                Label("lookup", systemImage: "arrow.right").foregroundStyle(.purple)
                Label("session", systemImage: "arrow.right").foregroundStyle(.green)
                Label("rekey", systemImage: "key.horizontal").foregroundStyle(.mint)
                Label("payload", systemImage: "arrow.right").foregroundStyle(.yellow)
            }
            Text((cohortState?.representedNodes ?? UInt64(state.nodes.values.filter(\.active).count)).formatted() + " nodes")
                .foregroundStyle(.secondary)
        }
        .font(.caption)
        .padding(10)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        .padding(12)
    }

    private func transportLegend(_ label: String, color: Color) -> some View {
        Label(label, systemImage: "circle.fill").foregroundStyle(color)
    }

    private func positions(in size: CGSize) -> [Int: CGPoint] {
        let active = state.nodes.values
            .filter { mode != .anomalies || anomalyNodeIDs.contains($0.id) }
            .sorted { $0.id < $1.id }
        let count = max(1, active.count)
        let radius = min(size.width, size.height) * 0.46
        let center = CGPoint(x: size.width / 2, y: size.height / 2)
        return Dictionary(uniqueKeysWithValues: active.enumerated().map { index, node in
            let fraction = sqrt((Double(index) + 0.5) / Double(count))
            let angle = Double(index) * 2.399_963_229_728_653
            return (node.id, CGPoint(
                x: center.x + cos(angle) * radius * fraction,
                y: center.y + sin(angle) * radius * fraction
            ))
        })
    }

    private func drawEdges(context: inout GraphicsContext, positions: [Int: CGPoint]) {
        var activePath = Path()
        var inactivePath = Path()
        var sharedPath = Path()
        var parentPath = Path()
        for edge in state.edges.values {
            if mode == .anomalies
                && (!anomalyNodeIDs.contains(edge.from) || !anomalyNodeIDs.contains(edge.to)) {
                continue
            }
            guard let from = positions[edge.from], let to = positions[edge.to] else { continue }
            if edge.active, mode == .sharedMedium, edge.sharedMediumGroup != nil {
                sharedPath.move(to: from)
                sharedPath.addLine(to: to)
            } else if edge.active {
                activePath.move(to: from)
                activePath.addLine(to: to)
            } else {
                inactivePath.move(to: from)
                inactivePath.addLine(to: to)
            }
        }
        if mode == .rootAdoption {
            for node in state.nodes.values {
                guard let parent = node.parent,
                      let from = positions[node.id],
                      let to = positions[parent] else { continue }
                parentPath.move(to: from)
                parentPath.addLine(to: to)
            }
        }
        let opacity = state.edges.count > 5_000 ? 0.055 : 0.13
        context.stroke(activePath, with: .color(.secondary.opacity(opacity)), lineWidth: 0.5)
        context.stroke(sharedPath, with: .color(.cyan.opacity(0.55)), lineWidth: 1.4)
        context.stroke(parentPath, with: .color(.orange.opacity(0.72)), lineWidth: 1.4)
        context.stroke(
            inactivePath,
            with: .color(.red.opacity(state.edges.count > 5_000 ? 0.16 : 0.48)),
            style: StrokeStyle(lineWidth: 1.2, dash: [4, 3])
        )
    }

    private func drawNodes(context: inout GraphicsContext, positions: [Int: CGPoint]) {
        let diameter = state.nodes.count > 5_000 ? 2.2 : state.nodes.count > 500 ? 3.2 : 6
        for node in state.nodes.values {
            guard let point = positions[node.id] else { continue }
            let rect = CGRect(x: point.x - diameter / 2, y: point.y - diameter / 2,
                              width: diameter, height: diameter)
            let color: Color
            if !node.active { color = .gray.opacity(0.18) }
            else if mode == .connectivity { color = transportColor(node.transportType) }
            else if mode == .sharedMedium { color = mediaZoneColor(node.mediaZone) }
            else if node.root == node.id { color = .orange }
            else { color = rootColor(node.root) }
            context.fill(Path(ellipseIn: rect), with: .color(color))
            if let at = state.lastRekeyAtNS[node.id],
               virtualTimeNS >= at,
               virtualTimeNS - at <= 250_000_000 {
                context.stroke(
                    Path(ellipseIn: rect.insetBy(dx: -5, dy: -5)),
                    with: .color(.mint.opacity(0.9)),
                    lineWidth: 2.5
                )
            }
            if let at = state.lastParentSwitchAtNS[node.id],
               virtualTimeNS >= at,
               virtualTimeNS - at <= 350_000_000 {
                context.stroke(
                    Path(ellipseIn: rect.insetBy(dx: -7, dy: -7)),
                    with: .color(.orange.opacity(0.95)),
                    lineWidth: 2.5
                )
            }
            if let at = state.lastSybilArrivalAtNS[node.id],
               virtualTimeNS >= at,
               virtualTimeNS - at <= 500_000_000 {
                context.stroke(
                    Path(ellipseIn: rect.insetBy(dx: -9, dy: -9)),
                    with: .color(.purple.opacity(0.95)),
                    lineWidth: 2.5
                )
            }
            if selection == node.id {
                context.stroke(
                    Path(ellipseIn: rect.insetBy(dx: -4, dy: -4)),
                    with: .color(.white),
                    lineWidth: 2
                )
            }
        }
    }

    private func drawTransmissions(context: inout GraphicsContext, positions: [Int: CGPoint]) {
        for flight in state.transmissions.values {
            if mode == .anomalies
                && (!anomalyNodeIDs.contains(flight.from)
                    || !anomalyNodeIDs.contains(flight.to)) {
                continue
            }
            guard let from = positions[flight.from], let to = positions[flight.to] else { continue }
            let span = max(1, flight.endNS - flight.startNS)
            let elapsed = virtualTimeNS > flight.startNS ? virtualTimeNS - flight.startNS : 0
            let progress = min(1, Double(elapsed) / Double(span))
            let point = CGPoint(
                x: from.x + (to.x - from.x) * progress,
                y: from.y + (to.y - from.y) * progress
            )
            var path = Path()
            path.move(to: from)
            path.addLine(to: to)
            let color: Color = switch flight.plane {
            case "data": .yellow
            case "bloom": .cyan
            case "lookup": .purple
            case "session": .green
            default: .pink
            }
            context.stroke(path, with: .color(color.opacity(0.42)), lineWidth: 1.5)
            context.fill(
                Path(ellipseIn: CGRect(x: point.x - 3, y: point.y - 3, width: 6, height: 6)),
                with: .color(color)
            )
        }
    }

    private func nearestNode(to point: CGPoint, positions: [Int: CGPoint]) -> Int? {
        positions.min { left, right in
            distance(left.value, point) < distance(right.value, point)
        }.flatMap { distance($0.value, point) < 18 ? $0.key : nil }
    }

    private func distance(_ left: CGPoint, _ right: CGPoint) -> Double {
        hypot(left.x - right.x, left.y - right.y)
    }

    private func rootColor(_ root: Int) -> Color {
        Color(hue: Double((root * 2_654_435_761) & 255) / 255, saturation: 0.68, brightness: 0.92)
    }

    private func transportColor(_ type: String) -> Color {
        switch type {
        case "wifi": .cyan
        case "ble": .blue
        case "tor": .purple
        case "ethernet": .green
        case "tcp": .indigo
        default: .teal
        }
    }

    private func mediaZoneColor(_ zone: String?) -> Color {
        guard let zone else { return .gray }
        let palette: [Color] = [.cyan, .orange, .purple, .green, .pink, .yellow, .blue, .mint]
        if let ordinal = Int(zone.split(separator: "-").last ?? "") {
            return palette[ordinal % palette.count]
        }
        let value = zone.utf8.reduce(0) { ($0 &* 31) &+ Int($1) }
        return palette[Int(value.magnitude % UInt(palette.count))]
    }
}
