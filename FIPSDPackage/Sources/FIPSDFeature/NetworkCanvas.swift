import SwiftUI

struct NetworkCanvas: View {
    let state: SimulationState
    let virtualTimeNS: UInt64
    @Binding var selection: Int?
    @Binding var mode: VisualizationMode
    let cohortState: CohortArtifactState?
    let anomalyNodeIDs: Set<Int>
    let displayBatch: DisplayProjectionBatch
    var body: some View {
        GeometryReader { geometry in
            let visibleNodeIDs = mode == .anomalies ? anomalyNodeIDs : nil
            let frame = RenderFrame(
                state: state,
                virtualTimeNS: virtualTimeNS,
                visibleNodeIDs: visibleNodeIDs
            )
            let positions = frame.positions(in: geometry.size)
            let cohorts = CohortLayout(state: state, size: geometry.size)
            InteractiveCanvasViewport { viewport, viewportSize in
                Canvas { context, _ in
                    context.concatenate(viewport.drawingTransform(in: viewportSize))
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
                        drawEdges(context: &context, frame: frame, positions: positions)
                        drawTransmissions(context: &context, frame: frame, positions: positions)
                        drawNodes(context: &context, positions: positions)
                    }
                }
                .contentShape(Rectangle())
                .gesture(SpatialTapGesture().onEnded { value in
                    let point = viewport.contentPoint(at: value.location, in: viewportSize)
                    selection = mode == .cohorts
                        ? cohorts.nearestRepresentative(to: point)
                        : nearestNode(to: point, positions: positions)
                })
            }
            .overlay(alignment: .topLeading) { legend }
            .overlay(alignment: .topTrailing) { projectionDisclosure }
            .overlay(alignment: .bottomLeading) { transferProgress }
        }
        .background(Color(nsColor: .windowBackgroundColor))
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

    private var projectionDisclosure: some View {
        VStack(alignment: .trailing, spacing: 3) {
            Text("Stable synthetic layout · distance is not a protocol metric")
            Text(displayBatch.label)
                .foregroundStyle(displayBatch.isCompressed ? .orange : .secondary)
        }
        .font(.caption2)
        .foregroundStyle(.secondary)
            .padding(8)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
            .padding(12)
    }

    private func drawEdges(
        context: inout GraphicsContext,
        frame: RenderFrame,
        positions: [Int: CGPoint]
    ) {
        var activePath = Path()
        var inactivePath = Path()
        var sharedPath = Path()
        var parentPath = Path()
        for item in frame.physicalLinks {
            let edge = item.edge
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
            for relation in frame.parentRelations {
                guard let from = positions[relation.child],
                      let to = positions[relation.parent] else { continue }
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

    private func drawTransmissions(
        context: inout GraphicsContext,
        frame: RenderFrame,
        positions: [Int: CGPoint]
    ) {
        for rendered in frame.transmissions {
            let flight = rendered.transmission
            guard let from = positions[flight.from], let to = positions[flight.to] else { continue }
            let progress = rendered.progress
            let point = CGPoint(
                x: from.x + (to.x - from.x) * progress,
                y: from.y + (to.y - from.y) * progress
            )
            let trailStart = max(0, progress - 0.07)
            let trail = CGPoint(
                x: from.x + (to.x - from.x) * trailStart,
                y: from.y + (to.y - from.y) * trailStart
            )
            var path = Path()
            path.move(to: trail)
            path.addLine(to: point)
            let color: Color = switch flight.plane {
            case "data": .yellow
            case "bloom": .cyan
            case "lookup": .purple
            case "session": .green
            default: .pink
            }
            context.stroke(path, with: .color(color.opacity(0.72)), lineWidth: 2)
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
