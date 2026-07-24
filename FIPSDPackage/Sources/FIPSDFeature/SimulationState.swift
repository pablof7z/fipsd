import Foundation

struct SimulationState: Equatable, Sendable {
    var nodes: [Int: NodeState] = [:]
    var edges: [Int: EdgeState] = [:]
    var transmissions: [String: Transmission] = [:]
    var lastEvent: SimulationEvent?
    var deliveredFrames = 0
    var transmittedBytes = 0
    var queuePeakBytes = 0
    var usefulBytesDelivered = 0
    var flowsDelivered = 0
    var flowsRejected = 0
    var controlRejected = 0
    var bloomDelivered = 0
    var bloomRejected = 0
    var lookupDelivered = 0
    var lookupRejected = 0
    var sessionDelivered = 0
    var sessionRejected = 0
    var rekeysCompleted = 0
    var lastRekeyAtNS: [Int: UInt64] = [:]
    var lookupWaves = 0
    var coordinateCacheInvalidations = 0
    var failedTransportClasses: Set<String> = []
    var parentQualityPulses = 0
    var parentSwitchesSuppressed = 0
    var lastParentSwitchAtNS: [Int: UInt64] = [:]
    var authenticatedSybilArrivals = 0
    var lastSybilArrivalAtNS: [Int: UInt64] = [:]
    var cacheHits = 0
    var applicationTransfers: [String: ApplicationTransferState] = [:]

    mutating func apply(_ event: SimulationEvent) {
        lastEvent = event
        switch event.kind {
        case "input.initial-topology": applyTopology(event.data)
        case "input.descending-root-arrival", "input.node-arrived": applyArrival(event.data)
        case "input.authenticated-sybil-arrived":
            applyArrival(event.data)
            authenticatedSybilArrivals += 1
            if let node = event.data["node"]?.int { lastSybilArrivalAtNS[node] = event.timeNS }
        case "input.node-disappeared", "input.node-reappeared": applyLifecycle(event.data)
        case "input.network-partitioned", "input.network-merged": applyNetwork(event)
        case "input.link-conditions-changed", "input.link-conditions-restored":
            applyLinkConditions(event.data)
        case "input.session-rekey-wave": lastRekeyAtNS.removeAll(keepingCapacity: true)
        case "session.rekey-completed": applyRekey(event)
        case "input.coordinate-cache-expired":
            coordinateCacheInvalidations += event.data["invalidated_entries"]?.int ?? 0
        case "input.lookup-wave": lookupWaves += 1
        case "input.transport-class-failed", "input.transport-class-restored":
            applyTransportClass(event)
        case "input.parent-ancestry-swapped", "input.parent-quality-alternated":
            applyParentCost(event)
        case "tree-announce.due": applyTransmission(event, plane: "control")
        case "tree-announce.delivered": applyDelivery(event)
        case "data.flow-offered": applyFlowOffer(event)
        case "data.frame-due": applyTransmission(event, plane: "data")
        case "data.frame-delivered": applyDataDelivery(event)
        case "bloom.filter-due": applyTransmission(event, plane: "bloom")
        case "bloom.filter-delivered": applyBloomDelivery(event)
        case "lookup.frame-due": applyTransmission(event, plane: "lookup")
        case "lookup.frame-delivered": applyRecoveryDelivery(event, plane: "lookup")
        case "session.frame-due": applyTransmission(event, plane: "session")
        case "session.frame-delivered": applyRecoveryDelivery(event, plane: "session")
        default: break
        }
    }

    mutating func expireTransmissions(at timeNS: UInt64) {
        transmissions = transmissions.filter { $0.value.endNS >= timeNS }
    }

    private mutating func applyTopology(_ data: [String: JSONValue]) {
        nodes.removeAll(keepingCapacity: true)
        edges.removeAll(keepingCapacity: true)
        for item in data["nodes"]?.array ?? [] {
            guard let object = item.object,
                  let id = object["id"]?.int,
                  let address = object["address"]?.string,
                  let active = object["active"]?.bool,
                  let root = object["root"]?.int else { continue }
            nodes[id] = NodeState(
                id: id,
                address: address,
                active: active,
                root: root,
                parent: object["parent"]?.int,
                sequence: object["sequence"]?.int ?? 0,
                transportProfile: object["transport_profile"]?.string ?? "udp",
                transportType: object["transport_type"]?.string ?? "udp",
                bandwidthBPS: object["bandwidth_bps"]?.int ?? 0,
                latencyNS: object["latency_ns"]?.uint64 ?? 0,
                jitterNS: object["jitter_ns"]?.uint64 ?? 0,
                mtuBytes: object["mtu_bytes"]?.int ?? 0,
                mediaZone: object["media_zone"]?.string
            )
        }
        for item in data["edges"]?.array ?? [] {
            guard let object = item.object,
                  let id = object["id"]?.int,
                  let from = object["from"]?.int,
                  let to = object["to"]?.int else { continue }
            edges[id] = EdgeState(
                id: id, from: from, to: to,
                active: object["active"]?.bool ?? true,
                bandwidthBPS: object["bandwidth_bps"]?.int ?? 0,
                latencyNS: object["latency_ns"]?.uint64 ?? 0,
                jitterNS: object["jitter_ns"]?.uint64 ?? 0,
                lossPPM: object["loss_ppm"]?.int ?? 0,
                mtuBytes: object["mtu_bytes"]?.int ?? 0,
                queueBytes: object["queue_bytes"]?.int ?? 0,
                sharedMediumGroup: object["shared_medium_group"]?.int,
                parentCostPPM: object["parent_cost_ppm"]?.int ?? 1_000_000
            )
        }
    }

    private mutating func applyArrival(_ data: [String: JSONValue]) {
        guard let id = data["node"]?.int else { return }
        var node = nodes[id] ?? NodeState(
            id: id,
            address: data["address"]?.string ?? "",
            active: true,
            root: id,
            parent: nil,
            sequence: 1
        )
        node.active = true
        node.address = data["address"]?.string ?? node.address
        node.root = id
        node.parent = nil
        node.mediaZone = data["media_zone"]?.string ?? node.mediaZone
        nodes[id] = node
        let edgeIDs = data["edges"]?.array?.compactMap(\.int)
            ?? data["edge"]?.int.map { [$0] } ?? []
        let targets = data["targets"]?.array?.compactMap(\.int)
            ?? data["target"]?.int.map { [$0] } ?? []
        for (edgeID, target) in zip(edgeIDs, targets) {
            if var edge = edges[edgeID] {
                edge.active = true
                edges[edgeID] = edge
            } else {
                edges[edgeID] = EdgeState(
                    id: edgeID, from: id, to: target,
                    sharedMediumGroup: data["shared_medium_group"]?.int
                )
            }
        }
    }

    private mutating func applyLifecycle(_ data: [String: JSONValue]) {
        guard let id = data["node"]?.int, var node = nodes[id] else { return }
        node.active = data["active"]?.bool ?? node.active
        if node.active {
            node.root = id
            node.parent = nil
        }
        nodes[id] = node
    }

    private mutating func applyNetwork(_ event: SimulationEvent) {
        let active = event.kind == "input.network-merged"
        for item in event.data["changed_edges"]?.array ?? [] {
            guard let id = item.object?["id"]?.int, var edge = edges[id] else { continue }
            edge.active = active
            edges[id] = edge
        }
    }

    private mutating func applyLinkConditions(_ data: [String: JSONValue]) {
        guard let id = data["edge"]?.int,
              var edge = edges[id],
              let after = data["after"]?.object else { return }
        edge.bandwidthBPS = after["bandwidth_bps"]?.int ?? edge.bandwidthBPS
        edge.latencyNS = after["latency_ns"]?.uint64 ?? edge.latencyNS
        edge.lossPPM = after["loss_ppm"]?.int ?? edge.lossPPM
        edge.mtuBytes = after["mtu_bytes"]?.int ?? edge.mtuBytes
        edge.queueBytes = after["queue_bytes"]?.int ?? edge.queueBytes
        edges[id] = edge
    }

    private mutating func applyTransportClass(_ event: SimulationEvent) {
        guard let profile = event.data["profile"]?.string else { return }
        let active = event.kind == "input.transport-class-restored"
        if active {
            failedTransportClasses.remove(profile)
        } else {
            failedTransportClasses.insert(profile)
        }
        for item in event.data["changed_edges"]?.array ?? [] {
            guard let id = item.object?["id"]?.int, var edge = edges[id] else { continue }
            edge.active = active
            edges[id] = edge
        }
    }

    private mutating func applyRekey(_ event: SimulationEvent) {
        guard let source = event.data["source"]?.int else { return }
        rekeysCompleted += 1
        lastRekeyAtNS[source] = event.timeNS
    }

    private mutating func applyTransmission(_ event: SimulationEvent, plane: String) {
        guard let from = event.data["from"]?.int,
              let to = event.data["to"]?.int else { return }
        transmittedBytes += event.data["transport_bytes"]?.int ?? 0
        queuePeakBytes = max(queuePeakBytes, event.data["queue_occupancy_bytes"]?.int ?? 0)
        if event.data["rejected"]?.string != nil {
            switch plane {
            case "data": flowsRejected += 1
            case "bloom": bloomRejected += 1
            case "lookup": lookupRejected += 1
            case "session": sessionRejected += 1
            default: controlRejected += 1
            }
        }
        for delivery in event.data["deliveries"]?.array ?? [] {
            guard let value = delivery.object,
                  let end = value["deliver_at_ns"]?.uint64 else { continue }
            let copy = value["copy"]?.int ?? 0
            let id = "\(event.id):\(copy)"
            transmissions[id] = Transmission(
                id: id,
                from: from,
                to: to,
                startNS: event.timeNS,
                endNS: end,
                frameBytes: event.data["frame_bytes"]?.int ?? 0,
                copy: copy,
                plane: plane
            )
        }
    }

    private mutating func applyDelivery(_ event: SimulationEvent) {
        guard let from = event.data["from"]?.int,
              let to = event.data["to"]?.int else { return }
        deliveredFrames += 1
        let copy = event.data["copy"]?.int ?? 0
        if let dueEvent = event.causalParent {
            transmissions.removeValue(forKey: "\(dueEvent):\(copy)")
        } else {
            transmissions = transmissions.filter {
                !($0.value.from == from && $0.value.to == to && $0.value.copy == copy)
            }
        }
        guard var node = nodes[to] else { return }
        node.root = event.data["root_node"]?.int ?? node.root
        node.parent = event.data["parent"]?.int
        node.sequence = event.data["sequence"]?.int ?? node.sequence
        nodes[to] = node
    }

    private mutating func applyFlowOffer(_ event: SimulationEvent) {
        applyApplicationTransferOffer(event)
        if event.data["status"]?.string == "rejected" { flowsRejected += 1 }
        if event.data["cache"]?.string == "cache-hit" { cacheHits += 1 }
    }

    private mutating func applyDataDelivery(_ event: SimulationEvent) {
        applyApplicationTransferDelivery(event)
        guard let from = event.data["from"]?.int,
              let to = event.data["to"]?.int else { return }
        deliveredFrames += 1
        let copy = event.data["copy"]?.int ?? 0
        if let dueEvent = event.causalParent {
            transmissions.removeValue(forKey: "\(dueEvent):\(copy)")
        } else {
            transmissions = transmissions.filter {
                !($0.value.from == from && $0.value.to == to && $0.value.copy == copy)
            }
        }
        if event.data["final"]?.bool == true {
            flowsDelivered += 1
            usefulBytesDelivered += event.data["useful_bytes"]?.int ?? 0
        }
    }

    private mutating func applyBloomDelivery(_ event: SimulationEvent) {
        guard let from = event.data["from"]?.int,
              let to = event.data["to"]?.int else { return }
        bloomDelivered += 1
        deliveredFrames += 1
        let copy = event.data["copy"]?.int ?? 0
        transmissions = transmissions.filter {
            !($0.value.from == from && $0.value.to == to && $0.value.copy == copy)
        }
    }

    private mutating func applyRecoveryDelivery(_ event: SimulationEvent, plane: String) {
        guard let from = event.data["from"]?.int,
              let to = event.data["to"]?.int else { return }
        if plane == "lookup" { lookupDelivered += 1 }
        else { sessionDelivered += 1 }
        deliveredFrames += 1
        let copy = event.data["copy"]?.int ?? 0
        transmissions = transmissions.filter {
            !($0.value.from == from && $0.value.to == to && $0.value.copy == copy)
        }
    }
}
