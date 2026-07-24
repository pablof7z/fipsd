import Testing
@testable import FIPSDFeature

@Test func independentOracleCoversPulseAndSharedMediumOverlays() throws {
    let raw = syntheticRendererEvents()
    let events = try raw.map { try #require(SimulationEvent($0)) }
    var state = SimulationState()
    var oracle = IndependentRenderOracle()
    for (event, value) in zip(events, raw) {
        state.apply(event)
        oracle.apply(value)
    }
    let timeNS: UInt64 = 50
    state.expireTransmissions(at: timeNS)
    oracle.expire(at: timeNS)
    let anomalyNodes: Set<Int> = [0, 1]

    for mode in VisualizationMode.allCases {
        let frame = RenderFrame(
            state: state,
            virtualTimeNS: timeNS,
            visualizationMode: mode,
            anomalyNodeIDs: anomalyNodes
        )
        #expect(
            frame.semanticVisibleManifest == oracle.visibleManifest(
                mode: mode,
                virtualTimeNS: timeNS,
                anomalyNodeIDs: anomalyNodes
            )
        )
    }
    let root = RenderFrame(state: state, virtualTimeNS: timeNS)
    #expect(root.pulses.count == 3)
    #expect(root.physicalLinks.last?.edge.sharedMediumGroup == 9)
}

private func syntheticRendererEvents() -> [JSONValue] {
    [
        syntheticEvent(
            id: "topology",
            timeNS: 0,
            ordinal: 0,
            kind: "input.initial-topology",
            data: [
                "nodes": .array([
                    syntheticNode(id: 0, root: 0, zone: "wired"),
                    syntheticNode(id: 1, root: 1, zone: "wifi")
                ]),
                "edges": .array([
                    .object([
                        "id": .integer(0),
                        "from": .integer(0),
                        "to": .integer(1),
                        "active": .bool(true),
                        "shared_medium_group": .integer(7)
                    ])
                ])
            ]
        ),
        syntheticEvent(
            id: "parent",
            timeNS: 10,
            ordinal: 1,
            kind: "input.parent-quality-alternated",
            data: [
                "node": .integer(1),
                "new_parent": .integer(0),
                "switched": .bool(true)
            ]
        ),
        syntheticEvent(
            id: "rekey",
            timeNS: 20,
            ordinal: 2,
            kind: "session.rekey-completed",
            data: ["source": .integer(0)]
        ),
        syntheticEvent(
            id: "sybil",
            timeNS: 30,
            ordinal: 3,
            kind: "input.authenticated-sybil-arrived",
            data: [
                "node": .integer(2),
                "address": .string("2"),
                "edge": .integer(2),
                "target": .integer(0),
                "media_zone": .string("mesh"),
                "shared_medium_group": .integer(9)
            ]
        ),
        syntheticEvent(
            id: "flight",
            timeNS: 40,
            ordinal: 4,
            kind: "session.frame-due",
            data: [
                "from": .integer(0),
                "to": .integer(1),
                "frame_bytes": .integer(64),
                "deliveries": .array([
                    .object([
                        "copy": .integer(0),
                        "deliver_at_ns": .integer(140)
                    ])
                ])
            ]
        )
    ]
}

private func syntheticNode(
    id: Int,
    root: Int,
    zone: String
) -> JSONValue {
    .object([
        "id": .integer(Int64(id)),
        "address": .string(String(id)),
        "active": .bool(true),
        "root": .integer(Int64(root)),
        "parent": .null,
        "sequence": .integer(1),
        "transport_type": .string("udp"),
        "media_zone": .string(zone)
    ])
}

private func syntheticEvent(
    id: String,
    timeNS: Int64,
    ordinal: Int64,
    kind: String,
    data: [String: JSONValue]
) -> JSONValue {
    .object([
        "event_id": .string(id),
        "virtual_time_ns": .integer(timeNS),
        "ordinal": .integer(ordinal),
        "kind": .string(kind),
        "causal_parent": .null,
        "data": .object(data)
    ])
}
