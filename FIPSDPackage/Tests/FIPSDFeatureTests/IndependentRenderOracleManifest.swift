@testable import FIPSDFeature

struct SemanticNodeMark: Equatable {
    let id: Int
    let active: Bool
    let root: Int
    let parent: Int?
    let sequence: Int
    let transport: String
    let mediaZone: String?
}

struct SemanticLinkMark: Equatable {
    let id: Int
    let from: Int
    let to: Int
    let active: Bool
    let sharedMediumGroup: Int?
}

struct SemanticRouteMark: Equatable {
    let id: String
    let source: Int
    let destination: Int
    let path: [Int]
    let totalBytes: Int
    let offeredBytes: Int
    let deliveredBytes: Int
    let startedAtNS: UInt64
    let lastDeliveryNS: UInt64?
}

struct SemanticFlightMark: Equatable {
    let id: String
    let from: Int
    let to: Int
    let startNS: UInt64
    let endNS: UInt64
    let plane: String
    let progress: Double
}

struct SemanticPulseMark: Equatable {
    let nodeID: Int
    let kind: String
    let occurredAtNS: UInt64
    let progress: Double
}

struct SemanticCohortMark: Equatable {
    let key: String
    let nodeIDs: [Int]
    let activeNodes: Int
}

struct SemanticCohortFlightMark: Equatable {
    let from: String
    let to: String
    let plane: String
    let count: Int
    let meanProgress: Double
}

struct SemanticVisibleManifest: Equatable {
    let nodes: [SemanticNodeMark]
    let links: [SemanticLinkMark]
    let parents: [(Int, Int)]
    let routes: [SemanticRouteMark]
    let flights: [SemanticFlightMark]
    let pulses: [SemanticPulseMark]
    let cohorts: [SemanticCohortMark]
    let cohortFlights: [SemanticCohortFlightMark]

    static let empty = SemanticVisibleManifest(
        nodes: [], links: [], parents: [], routes: [], flights: [],
        pulses: [], cohorts: [], cohortFlights: []
    )

    static func == (
        left: SemanticVisibleManifest,
        right: SemanticVisibleManifest
    ) -> Bool {
        left.nodes == right.nodes
            && left.links == right.links
            && left.parents.elementsEqual(
                right.parents,
                by: { $0.0 == $1.0 && $0.1 == $1.1 }
            )
            && left.routes == right.routes
            && left.flights == right.flights
            && left.pulses == right.pulses
            && left.cohorts == right.cohorts
            && left.cohortFlights == right.cohortFlights
    }
}

extension IndependentRenderOracle {
    func visibleManifest(
        mode: VisualizationMode,
        virtualTimeNS: UInt64,
        anomalyNodeIDs: Set<Int>
    ) -> SemanticVisibleManifest {
        if mode == .cohorts {
            return cohortManifest(virtualTimeNS: virtualTimeNS)
        }
        let included = mode == .anomalies
            ? anomalyNodeIDs
            : Set(nodes.keys)
        let visibleNodes = nodes.values.filter {
            included.contains($0.id)
        }.sorted { $0.id < $1.id }
        let nodeMarks = visibleNodes.map {
            SemanticNodeMark(
                id: $0.id, active: $0.active, root: $0.root,
                parent: $0.parent, sequence: $0.sequence,
                transport: $0.transport, mediaZone: $0.mediaZone
            )
        }
        let linkMarks = edges.values.filter {
            included.contains($0.from) && included.contains($0.to)
        }.sorted { $0.id < $1.id }.map {
            SemanticLinkMark(
                id: $0.id, from: $0.from, to: $0.to,
                active: $0.active,
                sharedMediumGroup: $0.sharedMediumGroup
            )
        }
        let parentMarks: [(Int, Int)] = mode == .rootAdoption
            ? visibleNodes.compactMap {
                guard let parent = $0.parent,
                      included.contains(parent),
                      nodes[parent] != nil else { return nil }
                return ($0.id, parent)
            }.sorted { ($0.0, $0.1) < ($1.0, $1.1) }
            : []
        let routeMarks = transfers.values.filter {
            !$0.path.isEmpty && $0.path.allSatisfy {
                included.contains($0) && nodes[$0] != nil
            }
        }.sorted { $0.id < $1.id }.map {
            SemanticRouteMark(
                id: $0.id, source: $0.source,
                destination: $0.destination, path: $0.path,
                totalBytes: $0.totalBytes, offeredBytes: $0.offeredBytes,
                deliveredBytes: $0.deliveredBytes,
                startedAtNS: $0.startedAtNS,
                lastDeliveryNS: $0.lastDeliveryNS
            )
        }
        let flightMarks = visibleFlights(
            included: included,
            virtualTimeNS: virtualTimeNS
        )
        let pulseMarks = pulses.values.compactMap {
            pulse -> SemanticPulseMark? in
            guard included.contains(pulse.nodeID),
                  nodes[pulse.nodeID] != nil,
                  virtualTimeNS >= pulse.occurredAtNS else { return nil }
            let age = virtualTimeNS - pulse.occurredAtNS
            guard age <= pulse.durationNS else { return nil }
            return SemanticPulseMark(
                nodeID: pulse.nodeID, kind: pulse.kind,
                occurredAtNS: pulse.occurredAtNS,
                progress: Double(age) / Double(pulse.durationNS)
            )
        }.sorted {
            ($0.nodeID, $0.kind, $0.occurredAtNS)
                < ($1.nodeID, $1.kind, $1.occurredAtNS)
        }
        return SemanticVisibleManifest(
            nodes: nodeMarks, links: linkMarks, parents: parentMarks,
            routes: routeMarks, flights: flightMarks, pulses: pulseMarks,
            cohorts: [], cohortFlights: []
        )
    }

    private func visibleFlights(
        included: Set<Int>,
        virtualTimeNS: UInt64
    ) -> [SemanticFlightMark] {
        flights.values.filter {
            included.contains($0.from) && included.contains($0.to)
        }.sorted { $0.id < $1.id }.map {
            let span = max(1, $0.endNS - $0.startNS)
            let elapsed = virtualTimeNS > $0.startNS
                ? virtualTimeNS - $0.startNS
                : 0
            return SemanticFlightMark(
                id: $0.id, from: $0.from, to: $0.to,
                startNS: $0.startNS, endNS: $0.endNS, plane: $0.plane,
                progress: min(1, Double(elapsed) / Double(span))
            )
        }
    }

    private func cohortManifest(
        virtualTimeNS: UInt64
    ) -> SemanticVisibleManifest {
        let depths = nodeDepths()
        var members: [String: [OracleNode]] = [:]
        for node in nodes.values {
            let key = cohortKey(
                root: node.root,
                depth: min(7, (depths[node.id] ?? 0) / 4),
                transport: node.transport
            )
            members[key, default: []].append(node)
        }
        let cohorts = members.map { key, values in
            SemanticCohortMark(
                key: key, nodeIDs: values.map(\.id).sorted(),
                activeNodes: values.count(where: \.active)
            )
        }.sorted { $0.key < $1.key }
        let membership = Dictionary(
            uniqueKeysWithValues: cohorts.flatMap { cohort in
                cohort.nodeIDs.map { ($0, cohort.key) }
            }
        )
        var grouped: [String: [SemanticFlightMark]] = [:]
        for flight in visibleFlights(
            included: Set(nodes.keys),
            virtualTimeNS: virtualTimeNS
        ) {
            guard let from = membership[flight.from],
                  let to = membership[flight.to] else { continue }
            grouped["\(from)>\(to)>\(flight.plane)", default: []]
                .append(flight)
        }
        let cohortFlights = grouped.map { key, values in
            let parts = key.split(separator: ">", maxSplits: 2).map(String.init)
            return SemanticCohortFlightMark(
                from: parts[0], to: parts[1], plane: parts[2],
                count: values.count,
                meanProgress: values.map(\.progress).reduce(0, +)
                    / Double(values.count)
            )
        }.sorted {
            ($0.from, $0.to, $0.plane) < ($1.from, $1.to, $1.plane)
        }
        return SemanticVisibleManifest(
            nodes: [], links: [], parents: [], routes: [], flights: [],
            pulses: [], cohorts: cohorts, cohortFlights: cohortFlights
        )
    }

    private func nodeDepths() -> [Int: Int] {
        Dictionary(uniqueKeysWithValues: nodes.values.map { node in
            var current = node
            var visited = Set([node.id])
            var depth = 0
            while let parent = current.parent,
                  visited.insert(parent).inserted,
                  let next = nodes[parent] {
                depth += 1
                current = next
            }
            return (node.id, depth)
        })
    }

    private func cohortKey(
        root: Int,
        depth: Int,
        transport: String
    ) -> String {
        "\(root):\(depth):\(transport)"
    }
}
