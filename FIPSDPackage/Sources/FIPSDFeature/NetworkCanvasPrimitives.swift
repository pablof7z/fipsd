import SwiftUI

extension NetworkCanvas {
    func drawNodes(
        context: inout GraphicsContext,
        frame: RenderFrame,
        positions: [Int: CGPoint]
    ) {
        let diameter = frame.nodes.count > 5_000
            ? 2.2
            : frame.nodes.count > 500 ? 3.2 : 6
        for rendered in frame.nodes {
            let node = rendered.state
            guard let point = positions[node.id] else { continue }
            let rect = CGRect(
                x: point.x - diameter / 2,
                y: point.y - diameter / 2,
                width: diameter,
                height: diameter
            )
            let color: Color
            if !node.active { color = .gray.opacity(0.18) }
            else if mode == .connectivity {
                color = transportColor(node.transportType)
            } else if mode == .sharedMedium {
                color = mediaZoneColor(node.mediaZone)
            } else if node.root == node.id {
                color = .orange
            } else {
                color = rootColor(node.root)
            }
            context.fill(Path(ellipseIn: rect), with: .color(color))
            drawRecentPulses(
                context: &context,
                frame: frame,
                node: node,
                rect: rect
            )
            if selection == node.id {
                context.stroke(
                    Path(ellipseIn: rect.insetBy(dx: -4, dy: -4)),
                    with: .color(.white),
                    lineWidth: 2
                )
            }
        }
    }

    func drawTransmissions(
        context: inout GraphicsContext,
        frame: RenderFrame,
        positions: [Int: CGPoint]
    ) {
        for rendered in frame.transmissions {
            let flight = rendered.transmission
            guard let from = positions[flight.from],
                  let to = positions[flight.to] else { continue }
            let progress = rendered.progress
            let point = interpolated(from: from, to: to, progress: progress)
            let trail = interpolated(
                from: from,
                to: to,
                progress: max(0, progress - 0.07)
            )
            var path = Path()
            path.move(to: trail)
            path.addLine(to: point)
            let color = planeColor(flight.plane)
            context.stroke(
                path,
                with: .color(color.opacity(0.72)),
                lineWidth: 2
            )
            context.fill(
                Path(ellipseIn: CGRect(
                    x: point.x - 3,
                    y: point.y - 3,
                    width: 6,
                    height: 6
                )),
                with: .color(color)
            )
        }
    }

    func nearestNode(to point: CGPoint, positions: [Int: CGPoint]) -> Int? {
        positions.min {
            distance($0.value, point) < distance($1.value, point)
        }.flatMap { distance($0.value, point) < 18 ? $0.key : nil }
    }

    private func drawRecentPulses(
        context: inout GraphicsContext,
        frame: RenderFrame,
        node: NodeState,
        rect: CGRect
    ) {
        let pulses: [(UInt64?, UInt64, CGFloat, Color)] = [
            (state.lastRekeyAtNS[node.id], 250_000_000, 5, .mint),
            (state.lastParentSwitchAtNS[node.id], 350_000_000, 7, .orange),
            (state.lastSybilArrivalAtNS[node.id], 500_000_000, 9, .purple)
        ]
        for (time, duration, inset, color) in pulses {
            guard let time, frame.virtualTimeNS >= time,
                  frame.virtualTimeNS - time <= duration else { continue }
            context.stroke(
                Path(ellipseIn: rect.insetBy(dx: -inset, dy: -inset)),
                with: .color(color.opacity(0.92)),
                lineWidth: 2.5
            )
        }
    }

    private func interpolated(
        from: CGPoint,
        to: CGPoint,
        progress: Double
    ) -> CGPoint {
        CGPoint(
            x: from.x + (to.x - from.x) * progress,
            y: from.y + (to.y - from.y) * progress
        )
    }

    private func distance(_ left: CGPoint, _ right: CGPoint) -> Double {
        hypot(left.x - right.x, left.y - right.y)
    }

    private func rootColor(_ root: Int) -> Color {
        Color(
            hue: Double((root * 2_654_435_761) & 255) / 255,
            saturation: 0.68,
            brightness: 0.92
        )
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
        let palette: [Color] = [
            .cyan, .orange, .purple, .green, .pink, .yellow, .blue, .mint
        ]
        if let ordinal = Int(zone.split(separator: "-").last ?? "") {
            return palette[ordinal % palette.count]
        }
        let value = zone.utf8.reduce(0) { ($0 &* 31) &+ Int($1) }
        return palette[Int(value.magnitude % UInt(palette.count))]
    }

    private func planeColor(_ plane: String) -> Color {
        switch plane {
        case "data": .yellow
        case "bloom": .cyan
        case "lookup": .purple
        case "session": .green
        default: .pink
        }
    }
}
