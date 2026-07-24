import Foundation
import Testing
@testable import FIPSDFeature

@Test func parentControlsBecomeReplayableCampaignEventsAndTimers() throws {
    var configuration = CampaignConfiguration()
    configuration.parentEvents = [
        ParentIntervention(
            atSeconds: 2,
            action: "alternate-parent-quality",
            node: 3,
            cycles: 6
        )
    ]
    configuration.parentOscillationIntervalMilliseconds = 125
    configuration.parentHysteresisPercent = 15
    configuration.parentHoldDownMilliseconds = 750
    let data = try CampaignBuilder.data(for: configuration)
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let events = try #require(campaign["events"] as? [[String: Any]])
    let event = try #require(
        events.first { $0["action"] as? String == "alternate-parent-quality" }
    )
    #expect(event["target"] as? Int == 3)
    let eventParameters = try #require(event["parameters"] as? [String: Any])
    #expect(eventParameters["cycles"] as? Int == 6)
    #expect(eventParameters["interval"] as? String == "125000000ns")
    let protocolObject = try #require(campaign["protocol"] as? [String: Any])
    let protocolParameters = try #require(protocolObject["parameters"] as? [String: Any])
    #expect(protocolParameters["parent_hysteresis_ppm"] as? Int == 150_000)
    #expect(protocolParameters["parent_hold_down"] as? String == "750000000ns")
}

@Test func parentPulseProjectsImmediateVisibleTreeState() throws {
    let topology = try #require(SimulationEvent(.object([
        "event_id": .string("topology"), "virtual_time_ns": .integer(0),
        "ordinal": .integer(0), "kind": .string("input.initial-topology"),
        "causal_parent": .null, "data": .object([
            "nodes": .array((0..<3).map { id in .object([
                "id": .integer(Int64(id)), "address": .string("8\(id)"),
                "active": .bool(true), "root": .integer(0),
                "parent": id == 2 ? .integer(1) : .null, "sequence": .integer(1)
            ]) }),
            "edges": .array([
                .object(["id": .integer(0), "from": .integer(2), "to": .integer(1)]),
                .object(["id": .integer(1), "from": .integer(2), "to": .integer(0)])
            ])
        ])
    ])))
    let pulse = try #require(SimulationEvent(.object([
        "event_id": .string("pulse"), "virtual_time_ns": .integer(2_000_000_000),
        "ordinal": .integer(1), "kind": .string("input.parent-quality-alternated"),
        "causal_parent": .null, "data": .object([
            "node": .integer(2), "new_parent": .integer(0), "switched": .bool(true),
            "suppressed": .bool(false), "old_parent_edge": .integer(0),
            "preferred_parent_edge": .integer(1), "preferred_cost_ppm": .integer(1_000_000),
            "degraded_cost_ppm": .integer(6_000_000)
        ])
    ])))
    var state = SimulationState()
    state.apply(topology)
    state.apply(pulse)
    #expect(state.nodes[2]?.parent == 0)
    #expect(state.parentQualityPulses == 1)
    #expect(state.lastParentSwitchAtNS[2] == 2_000_000_000)
    #expect(state.edges[0]?.parentCostPPM == 6_000_000)
    #expect(state.edges[1]?.parentCostPPM == 1_000_000)
}
