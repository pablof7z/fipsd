import Testing
@testable import FIPSDFeature

@Test func arrivingNodeInheritsAuthoredMediaZone() throws {
    let event = try #require(SimulationEvent(.object([
        "event_id": .string("arrival-1"),
        "virtual_time_ns": .integer(1_000),
        "ordinal": .integer(1),
        "kind": .string("input.descending-root-arrival"),
        "causal_parent": .null,
        "data": .object([
            "node": .integer(9),
            "address": .string("7f"),
            "target": .integer(2),
            "edge": .integer(11),
            "media_zone": .string("zone-2"),
            "shared_medium_group": .integer(2)
        ])
    ])))

    var state = SimulationState()
    state.apply(event)

    #expect(state.nodes[9]?.mediaZone == "zone-2")
    #expect(state.edges[11]?.sharedMediumGroup == 2)
}
