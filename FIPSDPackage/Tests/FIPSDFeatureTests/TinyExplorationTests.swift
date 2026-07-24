import Foundation
import Testing
@testable import FIPSDFeature

@Test func tinyBuilderCreatesABoundedDefaultLifecycleProbe() throws {
    var configuration = CampaignConfiguration()
    configuration.nodes = 1_000
    configuration.arrivals = 8
    let data = try CampaignBuilder.tinyExplorationData(
        for: configuration, maximumNodes: 6
    )
    let campaign = try #require(
        JSONSerialization.jsonObject(with: data) as? [String: Any]
    )
    let scale = try #require(campaign["scale"] as? [String: Any])
    #expect(scale["nodes"] as? Int == 6)
    let events = try #require(campaign["events"] as? [[String: Any]])
    #expect(events.contains { $0["action"] as? String == "disappear-node" })
    #expect(events.contains { $0["action"] as? String == "reappear-node" })
}

@Test func tinySummaryRetainsCoverageAndCounterexampleOrder() {
    let summary = TinyExplorationSummary.parse([
        "exploration_id": .string("explore-1"),
        "fidelity": .string("exhaustive"),
        "action_count": .integer(2),
        "expected_permutations": .integer(2),
        "explored_permutations": .integer(2),
        "exhaustive": .bool(true),
        "terminal_states": .object(["state-a": .integer(1)]),
        "counterexamples": .array([.object([
            "id": .string("counterexample-1"),
            "action_order": .array([.string("recover"), .string("fail")]),
            "failure": .string("cannot recover active node")
        ])])
    ])
    #expect(summary.exhaustive)
    #expect(summary.expected == 2)
    #expect(summary.terminalStates == 1)
    #expect(summary.counterexamples.first?.order == ["recover", "fail"])
}
