import SwiftUI

struct NetworkCanvas: View {
    let frame: RenderFrame
    let onSelectionChange: @MainActor @Sendable (Int?) -> Void
    let onModeChange: @MainActor @Sendable (VisualizationMode) -> Void
    var mode: VisualizationMode { frame.visualizationMode }
    var body: some View {
        GeometryReader { geometry in
            let positions = frame.positions(in: geometry.size)
            let cohorts = CohortLayout(frame: frame, size: geometry.size)
            InteractiveCanvasViewport { viewport, viewportSize in
                Canvas { context, _ in
                    context.concatenate(viewport.drawingTransform(in: viewportSize))
                    if mode == .cohorts {
                        if !frame.artifactCohorts.isEmpty {
                            CohortArtifactCanvas(
                                frame: frame,
                                size: geometry.size
                            ).draw(context: &context)
                        } else {
                            cohorts.draw(context: &context)
                        }
                    } else {
                        drawEdges(context: &context, frame: frame, positions: positions)
                        drawTransmissions(
                            context: &context,
                            frame: frame,
                            positions: positions
                        )
                        drawNodes(
                            context: &context,
                            frame: frame,
                            positions: positions
                        )
                    }
                }
                .contentShape(Rectangle())
                .gesture(SpatialTapGesture().onEnded { value in
                    let point = viewport.contentPoint(at: value.location, in: viewportSize)
                    onSelectionChange(mode == .cohorts
                        ? cohorts.nearestRepresentative(to: point)
                        : nearestNode(to: point, positions: positions))
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
            Picker(
                "View",
                selection: Binding(
                    get: { mode },
                    set: { onModeChange($0) }
                )
            ) {
                ForEach(VisualizationMode.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .frame(width: 150).accessibilityIdentifier("visualizationModePicker")
            if mode == .cohorts {
                Text(
                    frame.artifactCohortFidelity
                        ?? "transport × root × depth cohorts"
                )
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
                Label("physical", systemImage: "minus").foregroundStyle(.secondary)
                Label("parent", systemImage: "arrow.up.right").foregroundStyle(.orange)
                Label("route", systemImage: "point.topleft.down.to.point.bottomright.curvepath")
                    .foregroundStyle(.indigo)
                Label("control frame", systemImage: "arrow.right").foregroundStyle(.pink)
                Label("Bloom", systemImage: "arrow.right").foregroundStyle(.cyan)
                Label("lookup", systemImage: "arrow.right").foregroundStyle(.purple)
                Label("session", systemImage: "arrow.right").foregroundStyle(.green)
                Label("rekey", systemImage: "key.horizontal").foregroundStyle(.mint)
                Label("payload", systemImage: "arrow.right").foregroundStyle(.yellow)
            }
            Text(frame.representedNodes.formatted() + " represented nodes")
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
            Text(frame.sourceFidelity)
            Text("Renderer: exact retained state · deterministic cohort aggregation")
            Text(frame.displayBatch.label)
                .foregroundStyle(frame.displayBatch.isCompressed ? .orange : .secondary)
            if frame.displayBatch.isCompressed {
                Text(
                    "\(frame.displayBatch.initiatingEventIDs.count.formatted()) causal "
                        + "entry event(s) retained in renderer evidence"
                )
                .foregroundStyle(.orange)
            }
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
        var routePath = Path()
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
        for route in frame.routes {
            let points = route.nodeIDs.compactMap { positions[$0] }
            guard let first = points.first else { continue }
            routePath.move(to: first)
            for point in points.dropFirst() { routePath.addLine(to: point) }
        }
        let sourceLinks = frame.reconciliation.sourcePhysicalLinks
        let opacity = sourceLinks > 5_000 ? 0.055 : 0.13
        context.stroke(activePath, with: .color(.secondary.opacity(opacity)), lineWidth: 0.5)
        context.stroke(sharedPath, with: .color(.cyan.opacity(0.55)), lineWidth: 1.4)
        context.stroke(parentPath, with: .color(.orange.opacity(0.72)), lineWidth: 1.4)
        context.stroke(
            routePath,
            with: .color(.indigo.opacity(0.9)),
            style: StrokeStyle(lineWidth: 1.8, dash: [3, 3])
        )
        context.stroke(
            inactivePath,
            with: .color(.red.opacity(sourceLinks > 5_000 ? 0.16 : 0.48)),
            style: StrokeStyle(lineWidth: 1.2, dash: [4, 3])
        )
    }

}
