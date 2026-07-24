import Testing
@testable import FIPSDFeature

@Test func rekeyCompletionsAreCountedAndRetainNodePulseTime() throws {
    let request = try #require(SimulationEvent(.object([
        "event_id": .string("rekey-request"),
        "virtual_time_ns": .integer(1_000),
        "ordinal": .integer(1),
        "kind": .string("input.session-rekey-wave"),
        "causal_parent": .null,
        "data": .object(["scheduled_rekeys": .integer(1)])
    ])))
    let completion = try #require(SimulationEvent(.object([
        "event_id": .string("rekey-complete"),
        "virtual_time_ns": .integer(1_100),
        "ordinal": .integer(2),
        "kind": .string("session.rekey-completed"),
        "causal_parent": .string("rekey-request"),
        "data": .object([
            "source": .integer(3),
            "destination": .integer(7),
            "crypto_fidelity": .string("operation-counted-no-wire-frame")
        ])
    ])))
    var state = SimulationState()
    state.lastRekeyAtNS[99] = 500
    state.apply(request)
    #expect(state.lastRekeyAtNS.isEmpty)
    state.apply(completion)
    #expect(state.rekeysCompleted == 1)
    #expect(state.lastRekeyAtNS[3] == 1_100)
}
