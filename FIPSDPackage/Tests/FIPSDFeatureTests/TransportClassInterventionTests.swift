import Foundation
import Testing
@testable import FIPSDFeature

@Test func transportClassFailureAndRestoreAreDurableCampaignEvents() throws {
    var configuration = CampaignConfiguration()
    configuration.transportClassEvents = [
        TransportClassIntervention(atSeconds: 2, profile: "tor", restore: false),
        TransportClassIntervention(atSeconds: 3, profile: "tor", restore: true)
    ]
    let data = try CampaignBuilder.data(for: configuration)
    let document = try #require(
        JSONSerialization.jsonObject(with: data) as? [String: Any]
    )
    let events = try #require(document["events"] as? [[String: Any]])
    let actions = events.compactMap { $0["action"] as? String }
    #expect(actions.contains("fail-transport-class"))
    #expect(actions.contains("restore-transport-class"))
    #expect(events.filter { $0["target"] as? String == "tor" }.count == 2)
}

@Test func transportClassEventsToggleRenderedEdges() throws {
    var state = SimulationState()
    state.edges[4] = EdgeState(id: 4, from: 1, to: 2)
    let failed = try event(
        id: "down", kind: "input.transport-class-failed", active: false
    )
    let restored = try event(
        id: "up", kind: "input.transport-class-restored", active: true
    )
    state.apply(failed)
    #expect(state.failedTransportClasses == ["tor"])
    #expect(state.edges[4]?.active == false)
    state.apply(restored)
    #expect(state.failedTransportClasses.isEmpty)
    #expect(state.edges[4]?.active == true)
}

private func event(id: String, kind: String, active: Bool) throws -> SimulationEvent {
    try #require(SimulationEvent(.object([
        "event_id": .string(id),
        "virtual_time_ns": .integer(active ? 200 : 100),
        "ordinal": .integer(active ? 2 : 1),
        "kind": .string(kind),
        "causal_parent": .null,
        "data": .object([
            "profile": .string("tor"),
            "changed_edges": .array([.object([
                "id": .integer(4), "from": .integer(1), "to": .integer(2)
            ])])
        ])
    ])))
}
