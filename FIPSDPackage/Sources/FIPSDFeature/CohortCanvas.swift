import SwiftUI

struct CohortKey: Hashable {
    let root: Int
    let depthBand: Int
    let transport: String
}

private struct CohortBucket {
    let key: CohortKey
    var nodes: [Int]
    var active: Int
}

struct CohortFlightKey: Hashable {
    let from: Int
    let to: Int
    let plane: String
}

struct CohortFlightAggregate: Equatable {
    let key: CohortFlightKey
    let count: Int
    let meanProgress: Double
}

struct CohortLayout {
    private let buckets: [CohortBucket]
    private let positions: [Int: CGPoint]
    private let nodeBuckets: [Int: Int]

    init(state: SimulationState, size: CGSize) {
        let depths = Self.depths(state.nodes)
        var grouped: [CohortKey: CohortBucket] = [:]
        for node in state.nodes.values {
            let key = CohortKey(
                root: node.root,
                depthBand: min(7, (depths[node.id] ?? 0) / 4),
                transport: node.transportType
            )
            grouped[key, default: CohortBucket(key: key, nodes: [], active: 0)].nodes.append(node.id)
            if node.active { grouped[key]!.active += 1 }
        }
        buckets = grouped.values.sorted {
            ($0.key.root, $0.key.depthBand, $0.key.transport)
                < ($1.key.root, $1.key.depthBand, $1.key.transport)
        }
        let margin: CGFloat = 54
        let width = max(1, size.width - margin * 2)
        let height = max(1, size.height - margin * 2)
        var points: [Int: CGPoint] = [:]
        var membership: [Int: Int] = [:]
        for (index, bucket) in buckets.enumerated() {
            let x = margin + width * Self.rootFraction(bucket.key.root)
            let y = margin + height * (CGFloat(bucket.key.depthBand) + 0.5) / 8
            let offset = Self.transportOffset(bucket.key.transport)
            points[index] = CGPoint(x: x + offset.x, y: y + offset.y)
            for node in bucket.nodes { membership[node] = index }
        }
        positions = points
        nodeBuckets = membership
    }

    func draw(context: inout GraphicsContext, state: SimulationState, virtualTimeNS: UInt64) {
        drawGrid(context: &context)
        drawFlights(context: &context, state: state, virtualTimeNS: virtualTimeNS)
        for (index, bucket) in buckets.enumerated() {
            guard let point = positions[index] else { continue }
            let diameter = min(44, 8 + log2(Double(max(1, bucket.nodes.count))) * 4)
            let rect = CGRect(
                x: point.x - diameter / 2,
                y: point.y - diameter / 2,
                width: diameter,
                height: diameter
            )
            let activeRatio = Double(bucket.active) / Double(max(1, bucket.nodes.count))
            context.fill(
                Path(ellipseIn: rect),
                with: .color(Self.transportColor(bucket.key.transport).opacity(0.25 + 0.75 * activeRatio))
            )
            context.draw(
                Text(bucket.nodes.count.formatted()).font(.caption2).foregroundStyle(.white),
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
                ? buckets[index].nodes.min()
                : nil
        }
    }

    func position(of nodeID: Int) -> CGPoint? {
        nodeBuckets[nodeID].flatMap { positions[$0] }
    }

    func flightAggregates(
        state: SimulationState,
        virtualTimeNS: UInt64
    ) -> [CohortFlightAggregate] {
        let grouped = Dictionary(grouping: state.transmissions.values) { flight in
            CohortFlightKey(
                from: nodeBuckets[flight.from] ?? -1,
                to: nodeBuckets[flight.to] ?? -1,
                plane: flight.plane
            )
        }
        return grouped.map { key, flights in
            let progress = flights.reduce(0.0) { result, flight in
                let span = max(1, flight.endNS - flight.startNS)
                let elapsed = virtualTimeNS > flight.startNS
                    ? virtualTimeNS - flight.startNS : 0
                return result + min(1, Double(elapsed) / Double(span))
            } / Double(flights.count)
            return CohortFlightAggregate(
                key: key,
                count: flights.count,
                meanProgress: progress
            )
        }.sorted {
            ($0.key.from, $0.key.to, $0.key.plane)
                < ($1.key.from, $1.key.to, $1.key.plane)
        }
    }

    private func drawGrid(context: inout GraphicsContext) {
        for (index, point) in positions {
            let bucket = buckets[index]
            context.draw(
                Text("d\(bucket.key.depthBand * 4)+")
                    .font(.system(size: 8))
                    .foregroundStyle(.secondary),
                at: CGPoint(x: point.x, y: point.y + 25)
            )
        }
    }

    private func drawFlights(
        context: inout GraphicsContext,
        state: SimulationState,
        virtualTimeNS: UInt64
    ) {
        for aggregate in flightAggregates(state: state, virtualTimeNS: virtualTimeNS) {
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

    private static func depths(_ nodes: [Int: NodeState]) -> [Int: Int] {
        var result: [Int: Int] = [:]
        for node in nodes.values {
            var current = node
            var visited = Set([node.id])
            var depth = 0
            while let parent = current.parent, visited.insert(parent).inserted,
                  let next = nodes[parent] {
                depth += 1
                current = next
            }
            result[node.id] = depth
        }
        return result
    }

    private static func transportOffset(_ transport: String) -> CGPoint {
        switch transport {
        case "wifi": CGPoint(x: -12, y: -8)
        case "ble": CGPoint(x: 12, y: -8)
        case "tor": CGPoint(x: -12, y: 8)
        case "ethernet": CGPoint(x: 12, y: 8)
        default: .zero
        }
    }

    private static func rootFraction(_ root: Int) -> CGFloat {
        var value = UInt64(bitPattern: Int64(root)) &+ 0x9E37_79B9_7F4A_7C15
        value = (value ^ (value >> 30)) &* 0xBF58_476D_1CE4_E5B9
        value = (value ^ (value >> 27)) &* 0x94D0_49BB_1331_11EB
        value ^= value >> 31
        return 0.04 + 0.92 * CGFloat(value & 0xFFFF) / 65_535
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
