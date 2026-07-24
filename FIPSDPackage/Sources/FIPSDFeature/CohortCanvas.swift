import SwiftUI

struct CohortLayout {
    private let frame: RenderFrame
    private let positions: [CohortKey: CGPoint]
    private let nodeBuckets: [Int: CohortKey]

    init(frame: RenderFrame, size: CGSize) {
        self.frame = frame
        let viewport = WorldViewport(points: frame.cohorts.map(\.worldPoint), in: size)
        positions = Dictionary(uniqueKeysWithValues: frame.cohorts.map {
            ($0.key, viewport.project($0.worldPoint))
        })
        nodeBuckets = Dictionary(
            uniqueKeysWithValues: frame.cohorts.flatMap { cohort in
                cohort.nodeIDs.map { ($0, cohort.key) }
            }
        )
    }

    func draw(context: inout GraphicsContext) {
        drawGrid(context: &context)
        drawFlights(context: &context)
        for bucket in frame.cohorts {
            guard let point = positions[bucket.key] else { continue }
            let diameter = RenderMarkMetrics.cohortDiameter(nodeCount: bucket.nodeIDs.count)
            let rect = CGRect(
                x: point.x - diameter / 2,
                y: point.y - diameter / 2,
                width: diameter,
                height: diameter
            )
            let activeRatio = Double(bucket.activeNodes)
                / Double(max(1, bucket.nodeIDs.count))
            context.fill(
                Path(ellipseIn: rect),
                with: .color(Self.transportColor(bucket.key.transport).opacity(0.25 + 0.75 * activeRatio))
            )
            context.draw(
                Text(bucket.nodeIDs.count.formatted()).font(.caption2).foregroundStyle(.white),
                at: point
            )
        }
    }

    func nearestRepresentative(to point: CGPoint) -> Int? {
        positions.min {
            hypot($0.value.x - point.x, $0.value.y - point.y)
                < hypot($1.value.x - point.x, $1.value.y - point.y)
        }.flatMap { index, position in
            hypot(position.x - point.x, position.y - point.y) < 28
                ? frame.cohorts.first { $0.key == index }?.nodeIDs.min()
                : nil
        }
    }

    func position(of nodeID: Int) -> CGPoint? {
        nodeBuckets[nodeID].flatMap { positions[$0] }
    }

    func worldPoint(of nodeID: Int) -> RenderWorldPoint? {
        nodeBuckets[nodeID].flatMap { key in
            frame.cohorts.first { $0.key == key }?.worldPoint
        }
    }

    var flightAggregates: [CohortFlightAggregate] {
        frame.cohortTransmissions
    }

    private func drawGrid(context: inout GraphicsContext) {
        // Iterate cohorts in their stable sorted order (not dictionary order) and
        // skip labels that would land on top of one already drawn, so overlapping
        // cohorts don't smear their depth bands into an unreadable blur.
        var drawn: [CGPoint] = []
        for bucket in frame.cohorts {
            guard let point = positions[bucket.key] else { continue }
            let anchor = CGPoint(x: point.x, y: point.y + 25)
            if drawn.contains(where: { hypot($0.x - anchor.x, $0.y - anchor.y) < 22 }) {
                continue
            }
            drawn.append(anchor)
            context.draw(
                Text("d\(bucket.key.depthBand * 4)+")
                    .font(.system(size: 8))
                    .foregroundStyle(.secondary),
                at: anchor
            )
        }
    }

    private func drawFlights(context: inout GraphicsContext) {
        for aggregate in frame.cohortTransmissions {
            let key = aggregate.key
            guard let from = positions[key.from], let to = positions[key.to] else { continue }
            let progress = aggregate.meanProgress
            let trailProgress = max(0, progress - 0.07)
            let trail = CGPoint(
                x: from.x + (to.x - from.x) * trailProgress,
                y: from.y + (to.y - from.y) * trailProgress
            )
            var path = Path()
            let point = CGPoint(
                x: from.x + (to.x - from.x) * progress,
                y: from.y + (to.y - from.y) * progress
            )
            path.move(to: trail)
            path.addLine(to: point)
            let color = Self.planeColor(key.plane)
            let width = min(8, 1 + log2(Double(aggregate.count + 1)))
            context.stroke(path, with: .color(color.opacity(0.72)), lineWidth: width)
            let dot = CGRect(x: point.x - 3, y: point.y - 3, width: 6, height: 6)
            context.fill(Path(ellipseIn: dot), with: .color(color))
            if aggregate.count > 1 {
                context.draw(
                    Text("×\(aggregate.count)").font(.system(size: 8)).foregroundStyle(color),
                    at: CGPoint(x: point.x + 10, y: point.y - 8)
                )
            }
        }
    }

    private static func transportColor(_ transport: String) -> Color {
        switch transport {
        case "wifi": .cyan
        case "ble": .blue
        case "tor": .purple
        case "ethernet": .green
        default: .teal
        }
    }

    private static func planeColor(_ plane: String) -> Color {
        switch plane {
        case "data": .yellow
        case "bloom": .cyan
        case "lookup": .purple
        case "session": .green
        default: .pink
        }
    }
}
