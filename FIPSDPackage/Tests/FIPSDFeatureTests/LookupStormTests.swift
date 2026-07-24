import Foundation
import Testing
@testable import FIPSDFeature

@Test func lookupStormIsAuthoredAsOrderedReplayableInputs() throws {
    var configuration = CampaignConfiguration()
    configuration.lookupStorms = [LookupStormIntervention(atSeconds: 2.5, count: 37)]
    let data = try CampaignBuilder.data(for: configuration)
    let document = try #require(
        JSONSerialization.jsonObject(with: data) as? [String: Any]
    )
    let events = try #require(document["events"] as? [[String: Any]])
    let expiry = try #require(
        events.first { $0["action"] as? String == "expire-coordinate-cache" }
    )
    let wave = try #require(
        events.first { $0["action"] as? String == "simultaneous-lookups" }
    )
    #expect(expiry["at"] as? String == "2500000000ns")
    #expect(wave["at"] as? String == "2500000000ns")
    #expect((wave["parameters"] as? [String: Any])?["count"] as? Int == 37)
}

@Test func lookupStormInputsProjectIntoVisibleCounters() throws {
    let expiry = try #require(SimulationEvent(.object([
        "event_id": .string("expiry"),
        "virtual_time_ns": .integer(100),
        "ordinal": .integer(1),
        "kind": .string("input.coordinate-cache-expired"),
        "causal_parent": .null,
        "data": .object(["invalidated_entries": .integer(12)])
    ])))
    let wave = try #require(SimulationEvent(.object([
        "event_id": .string("wave"),
        "virtual_time_ns": .integer(100),
        "ordinal": .integer(2),
        "kind": .string("input.lookup-wave"),
        "causal_parent": .null,
        "data": .object(["scheduled_lookups": .integer(20)])
    ])))
    var state = SimulationState()
    state.apply(expiry)
    state.apply(wave)
    #expect(state.coordinateCacheInvalidations == 12)
    #expect(state.lookupWaves == 1)
}
