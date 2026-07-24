import Foundation
import Testing
@testable import FIPSDFeature

@Test func sybilControlsAuthorAuthenticatedBudgetedEvents() throws {
    var configuration = CampaignConfiguration()
    configuration.nodes = 20
    configuration.arrivals = 0
    configuration.sybilEvents = [
        SybilIntervention(
            atSeconds: 2,
            count: 4,
            intervalMilliseconds: 100,
            attachment: "hub",
            rootGrinding: true
        )
    ]
    let data = try CampaignBuilder.data(for: configuration)
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let adversaries = try #require(campaign["adversaries"] as? [String: Any])
    #expect(adversaries["mode"] as? String == "authenticated-protocol-valid")
    let budgets = try #require(adversaries["budgets"] as? [String: Any])
    #expect(budgets["identities"] as? Int == 4)
    let events = try #require(campaign["events"] as? [[String: Any]])
    let event = try #require(
        events.first { $0["action"] as? String == "attach-authenticated-sybils" }
    )
    let parameters = try #require(event["parameters"] as? [String: Any])
    #expect(parameters["count"] as? Int == 4)
    #expect(parameters["interval"] as? String == "100000000ns")
    #expect(parameters["attachment"] as? String == "hub")
    #expect(parameters["address_policy"] as? String == "lower-than-current-root")
}

@Test func sybilArrivalAppearsAsANodeAndVisibleCounter() throws {
    let topology = try #require(SimulationEvent(.object([
        "event_id": .string("topology"), "virtual_time_ns": .integer(0),
        "ordinal": .integer(0), "kind": .string("input.initial-topology"),
        "causal_parent": .null, "data": .object([
            "nodes": .array([.object([
                "id": .integer(9), "address": .string("89"),
                "active": .bool(false), "root": .integer(9),
                "parent": .null, "sequence": .integer(1)
            ])]),
            "edges": .array([])
        ])
    ])))
    let arrival = try #require(SimulationEvent(.object([
        "event_id": .string("sybil"), "virtual_time_ns": .integer(2_000_000_000),
        "ordinal": .integer(1), "kind": .string("input.authenticated-sybil-arrived"),
        "causal_parent": .null, "data": .object([
            "node": .integer(9), "address": .string("7f"), "active": .bool(true),
            "root": .integer(9), "parent": .null, "sequence": .integer(2),
            "target": .integer(0), "edge": .integer(4), "authenticated": .bool(true)
        ])
    ])))
    var state = SimulationState()
    state.apply(topology)
    state.apply(arrival)
    #expect(state.nodes[9]?.active == true)
    #expect(state.nodes[9]?.address == "7f")
    #expect(state.authenticatedSybilArrivals == 1)
    #expect(state.lastSybilArrivalAtNS[9] == 2_000_000_000)
    #expect(state.edges[4]?.from == 9)
}
