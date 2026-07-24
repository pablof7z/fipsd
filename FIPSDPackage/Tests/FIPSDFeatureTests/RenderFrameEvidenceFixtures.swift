@testable import FIPSDFeature

func evidenceState() -> SimulationState {
    var state = SimulationState()
    state.nodes[0] = evidenceNode(0)
    state.nodes[1] = evidenceNode(1)
    state.edges[0] = EdgeState(id: 0, from: 0, to: 1)
    state.applicationTransfers["download"] = ApplicationTransferState(
        id: "download",
        source: 0,
        destination: 1,
        path: [0, 1],
        totalBytes: 100,
        offeredBytes: 100,
        deliveredBytes: 0,
        startedAtNS: 0
    )
    return state
}

private func evidenceNode(_ id: Int) -> NodeState {
    NodeState(
        id: id,
        address: "\(id)",
        active: true,
        root: id,
        parent: nil,
        sequence: 1
    )
}

func evidenceEvent(
    id: String,
    ordinal: UInt64,
    kind: String,
    causalParent: String? = nil
) -> SimulationEvent? {
    SimulationEvent(.object([
        "event_id": .string(id),
        "virtual_time_ns": .integer(Int64(ordinal * 10)),
        "ordinal": .integer(Int64(ordinal)),
        "kind": .string(kind),
        "causal_parent": causalParent.map(JSONValue.string) ?? .null,
        "data": .object([:])
    ]))
}
