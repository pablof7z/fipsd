import Foundation

struct CohortKey: Hashable, Equatable, Sendable {
    let root: Int
    let depthBand: Int
    let transport: String
}

struct RenderCohort: Equatable, Sendable {
    let key: CohortKey
    let nodeIDs: [Int]
    let activeNodes: Int
    let worldPoint: RenderWorldPoint
}

struct CohortFlightKey: Hashable, Equatable, Sendable {
    let from: CohortKey
    let to: CohortKey
    let plane: String
}

struct CohortFlightAggregate: Equatable, Sendable {
    let key: CohortFlightKey
    let count: Int
    let meanProgress: Double
}

struct CohortProjection: Equatable, Sendable {
    let cohorts: [RenderCohort]
    let transmissions: [CohortFlightAggregate]

    init(nodes: [RenderNode], transmissions: [RenderTransmission]) {
        let states = Dictionary(uniqueKeysWithValues: nodes.map { ($0.state.id, $0.state) })
        let depths = Self.depths(states)
        var members: [CohortKey: [RenderNode]] = [:]
        for node in nodes {
            let key = CohortKey(
                root: node.state.root,
                depthBand: min(7, (depths[node.state.id] ?? 0) / 4),
                transport: node.state.transportType
            )
            members[key, default: []].append(node)
        }
        cohorts = members.map { key, groupedNodes in
            RenderCohort(
                key: key,
                nodeIDs: groupedNodes.map(\.state.id).sorted(),
                activeNodes: groupedNodes.count(where: \.state.active),
                worldPoint: Self.worldPoint(for: key)
            )
        }.sorted {
            ($0.key.root, $0.key.depthBand, $0.key.transport)
                < ($1.key.root, $1.key.depthBand, $1.key.transport)
        }
        let membership = Dictionary(
            uniqueKeysWithValues: cohorts.flatMap { cohort in
                cohort.nodeIDs.map { ($0, cohort.key) }
            }
        )
        let groupedFlights = Dictionary(grouping: transmissions) {
            flight -> CohortFlightKey? in
            guard let from = membership[flight.transmission.from],
                  let to = membership[flight.transmission.to] else { return nil }
            return CohortFlightKey(
                from: from,
                to: to,
                plane: flight.transmission.plane
            )
        }
        self.transmissions = groupedFlights.compactMap { key, flights in
            guard let key else { return nil }
            return CohortFlightAggregate(
                key: key,
                count: flights.count,
                meanProgress: flights.map(\.progress).reduce(0, +) / Double(flights.count)
            )
        }.sorted {
            let left = $0.key
            let right = $1.key
            if left.from != right.from {
                return Self.keyLess(left.from, right.from)
            }
            if left.to != right.to {
                return Self.keyLess(left.to, right.to)
            }
            return left.plane < right.plane
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

    private static func keyLess(_ left: CohortKey, _ right: CohortKey) -> Bool {
        (left.root, left.depthBand, left.transport)
            < (right.root, right.depthBand, right.transport)
    }

    private static func worldPoint(for key: CohortKey) -> RenderWorldPoint {
        let root = rootFraction(key.root) * 2 - 1
        let depth = (Double(key.depthBand) + 0.5) / 8 * 2 - 1
        let offset = transportOffset(key.transport)
        return RenderWorldPoint(x: root + offset.x, y: depth + offset.y)
    }

    private static func transportOffset(_ transport: String) -> RenderWorldPoint {
        switch transport {
        case "wifi": RenderWorldPoint(x: -0.025, y: -0.03)
        case "ble": RenderWorldPoint(x: 0.025, y: -0.03)
        case "tor": RenderWorldPoint(x: -0.025, y: 0.03)
        case "ethernet": RenderWorldPoint(x: 0.025, y: 0.03)
        default: RenderWorldPoint(x: 0, y: 0)
        }
    }

    private static func rootFraction(_ root: Int) -> Double {
        var value = UInt64(bitPattern: Int64(root)) &+ 0x9E37_79B9_7F4A_7C15
        value = (value ^ (value >> 30)) &* 0xBF58_476D_1CE4_E5B9
        value = (value ^ (value >> 27)) &* 0x94D0_49BB_1331_11EB
        value ^= value >> 31
        return 0.04 + 0.92 * Double(value & 0xFFFF) / 65_535
    }
}
