@testable import FIPSDFeature

struct OracleNode: Equatable {
    let id: Int
    var active: Bool
    var root: Int
    var parent: Int?
    var sequence: Int
    var transport: String
    var mediaZone: String?
}

struct OracleEdge: Equatable {
    let id: Int
    let from: Int
    let to: Int
    var active: Bool
    var sharedMediumGroup: Int?
}

struct OracleFlight: Equatable {
    let id: String
    let from: Int
    let to: Int
    let startNS: UInt64
    let endNS: UInt64
    let plane: String
}

struct OracleTransfer: Equatable {
    let id: String
    let source: Int
    let destination: Int
    var path: [Int]
    let totalBytes: Int
    var offeredBytes: Int
    var deliveredBytes: Int
    var deliveredChunks: Set<Int>
    let startedAtNS: UInt64
    var lastDeliveryNS: UInt64?
}

struct OraclePulse: Equatable {
    let nodeID: Int
    let kind: String
    let occurredAtNS: UInt64
    let durationNS: UInt64
}

struct IndependentRenderOracle {
    var nodes: [Int: OracleNode] = [:]
    var edges: [Int: OracleEdge] = [:]
    var flights: [String: OracleFlight] = [:]
    var transfers: [String: OracleTransfer] = [:]
    var pulses: [String: OraclePulse] = [:]

    mutating func apply(_ value: JSONValue) {
        guard let event = value.object,
              let eventID = event["event_id"]?.string,
              let timeNS = event["virtual_time_ns"]?.uint64,
              let kind = event["kind"]?.string,
              let data = event["data"]?.object else { return }
        let parent = event["causal_parent"]?.string
        switch kind {
        case "input.initial-topology":
            applyTopology(data)
        case "input.descending-root-arrival", "input.node-arrived":
            applyArrival(data)
        case "input.authenticated-sybil-arrived":
            applyArrival(data)
            addPulse(node: data["node"]?.int, kind: "authenticated-sybil-arrival",
                     timeNS: timeNS, durationNS: 500_000_000)
        case "input.node-disappeared", "input.node-reappeared":
            applyLifecycle(data)
        case "input.network-partitioned", "input.network-merged":
            applyNetwork(data, active: kind == "input.network-merged")
        case "input.transport-class-failed", "input.transport-class-restored":
            applyNetwork(data, active: kind == "input.transport-class-restored")
        case "input.parent-ancestry-swapped", "input.parent-quality-alternated":
            applyParent(data, timeNS: timeNS)
        case "input.session-rekey-wave":
            pulses = pulses.filter { $0.value.kind != "rekey" }
        case "session.rekey-completed":
            addPulse(node: data["source"]?.int, kind: "rekey",
                     timeNS: timeNS, durationNS: 250_000_000)
        case "tree-announce.due":
            applyDue(eventID, data: data, timeNS: timeNS, plane: "control")
        case "data.frame-due":
            applyDue(eventID, data: data, timeNS: timeNS, plane: "data")
        case "bloom.filter-due":
            applyDue(eventID, data: data, timeNS: timeNS, plane: "bloom")
        case "lookup.frame-due":
            applyDue(eventID, data: data, timeNS: timeNS, plane: "lookup")
        case "session.frame-due":
            applyDue(eventID, data: data, timeNS: timeNS, plane: "session")
        case "tree-announce.delivered":
            removeFlight(data: data, causalParent: parent)
            applyTreeDelivery(data)
        case "data.flow-offered":
            applyTransferOffer(data, timeNS: timeNS)
        case "data.frame-delivered":
            removeFlight(data: data, causalParent: parent)
            applyTransferDelivery(data, timeNS: timeNS)
        case "bloom.filter-delivered", "lookup.frame-delivered",
             "session.frame-delivered":
            removeFlight(data: data, causalParent: nil)
        default:
            break
        }
    }

    mutating func expire(at timeNS: UInt64) {
        flights = flights.filter { $0.value.endNS >= timeNS }
    }

    private mutating func applyTopology(_ data: [String: JSONValue]) {
        nodes = [:]
        edges = [:]
        for value in data["nodes"]?.array ?? [] {
            guard let item = value.object, let id = item["id"]?.int,
                  let active = item["active"]?.bool,
                  let root = item["root"]?.int else { continue }
            nodes[id] = OracleNode(
                id: id, active: active, root: root,
                parent: item["parent"]?.int,
                sequence: item["sequence"]?.int ?? 0,
                transport: item["transport_type"]?.string ?? "udp",
                mediaZone: item["media_zone"]?.string
            )
        }
        for value in data["edges"]?.array ?? [] {
            guard let item = value.object, let id = item["id"]?.int,
                  let from = item["from"]?.int,
                  let to = item["to"]?.int else { continue }
            edges[id] = OracleEdge(
                id: id, from: from, to: to,
                active: item["active"]?.bool ?? true,
                sharedMediumGroup: item["shared_medium_group"]?.int
            )
        }
    }

    private mutating func applyArrival(_ data: [String: JSONValue]) {
        guard let id = data["node"]?.int else { return }
        var node = nodes[id] ?? OracleNode(
            id: id, active: true, root: id, parent: nil, sequence: 1,
            transport: "udp", mediaZone: nil
        )
        node.active = true
        node.root = id
        node.parent = nil
        node.mediaZone = data["media_zone"]?.string ?? node.mediaZone
        nodes[id] = node
        let ids = data["edges"]?.array?.compactMap(\.int)
            ?? data["edge"]?.int.map { [$0] } ?? []
        let targets = data["targets"]?.array?.compactMap(\.int)
            ?? data["target"]?.int.map { [$0] } ?? []
        for (edgeID, target) in zip(ids, targets) {
            if var edge = edges[edgeID] {
                edge.active = true
                edges[edgeID] = edge
            } else {
                edges[edgeID] = OracleEdge(
                    id: edgeID, from: id, to: target, active: true,
                    sharedMediumGroup: data["shared_medium_group"]?.int
                )
            }
        }
    }

    private mutating func applyLifecycle(_ data: [String: JSONValue]) {
        guard let id = data["node"]?.int, var node = nodes[id] else { return }
        node.active = data["active"]?.bool ?? node.active
        if node.active { node.root = id; node.parent = nil }
        nodes[id] = node
    }

    private mutating func applyNetwork(
        _ data: [String: JSONValue],
        active: Bool
    ) {
        for value in data["changed_edges"]?.array ?? [] {
            guard let id = value.object?["id"]?.int,
                  var edge = edges[id] else { continue }
            edge.active = active
            edges[id] = edge
        }
    }

    private mutating func applyParent(
        _ data: [String: JSONValue],
        timeNS: UInt64
    ) {
        guard let id = data["node"]?.int, var node = nodes[id] else { return }
        if let parent = data["new_parent"]?.int { node.parent = parent }
        if data["switched"]?.bool == true {
            node.sequence += 1
            addPulse(node: id, kind: "parent-switch",
                     timeNS: timeNS, durationNS: 350_000_000)
        }
        nodes[id] = node
    }

    private mutating func applyTreeDelivery(_ data: [String: JSONValue]) {
        guard let id = data["to"]?.int, var node = nodes[id] else { return }
        node.root = data["root_node"]?.int ?? node.root
        node.parent = data["parent"]?.int
        node.sequence = data["sequence"]?.int ?? node.sequence
        nodes[id] = node
    }

    private mutating func addPulse(
        node: Int?,
        kind: String,
        timeNS: UInt64,
        durationNS: UInt64
    ) {
        guard let node else { return }
        pulses["\(node):\(kind)"] = OraclePulse(
            nodeID: node, kind: kind, occurredAtNS: timeNS,
            durationNS: durationNS
        )
    }
}
