import Foundation
import Testing
@testable import FIPSDFeature

@Test func authoringProviderExposesEveryClaudeModelAlias() {
    #expect(AuthoringProvider.claudeSonnet.executableName == "claude")
    #expect(AuthoringProvider.claudeSonnet.claudeModel == "sonnet")
    #expect(AuthoringProvider.claudeHaiku.claudeModel == "haiku")
    #expect(AuthoringProvider.claudeOpus.claudeModel == "opus")
    #expect(AuthoringProvider.codex.claudeModel == nil)
}

@Test func replacementBridgeIntentUsesTheRenderedTransferRoute() throws {
    let campaign = try #require(ExplicitTransferIntent.parse(
        "Three nodes download a 500 MB file from the first to the last."
    )).applying(to: CampaignBuilder.data(for: CampaignConfiguration()))
    let context = try JSONSerialization.data(withJSONObject: [
        "application_transfers": [[
            "id": "requested-download",
            "source": 0,
            "destination": 2,
            "path": [0, 1, 2]
        ]]
    ])
    let prompt = """
    A new node joins the two nodes exchanging traffic. After 10 seconds, the node
    that was the bridge between them disappears.
    """
    let intent = try #require(
        BridgeReplacementIntent.parse(prompt, renderedState: context)
    )
    #expect(intent.source == 0)
    #expect(intent.destination == 2)
    #expect(intent.bridge == 1)
    #expect(intent.removalDelayNS == 10_000_000_000)
    let amended = try intent.applying(
        to: campaign,
        at: 6_000_000_000,
        realizedArrivals: 0
    )
    let object = try #require(
        JSONSerialization.jsonObject(with: amended) as? [String: Any]
    )
    let events = try #require(object["events"] as? [[String: Any]])
    let arrival = try #require(events.first { $0["action"] as? String == "introduce-node" })
    #expect(arrival["at"] as? String == "6000000000ns")
    let parameters = try #require(arrival["parameters"] as? [String: Any])
    #expect(parameters["attachments"] as? [Int] == [0, 2])
    let departure = try #require(events.first { $0["action"] as? String == "disappear-node" })
    #expect(departure["target"] as? Int == 1)
    #expect(departure["at"] as? String == "16000000000ns")
}

@Test func renderedTransferUsesLatestRouteAndUniqueDeliveredBytes() throws {
    func event(
        _ kind: String,
        id: String,
        data: [String: JSONValue]
    ) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string(id),
            "virtual_time_ns": .integer(1),
            "ordinal": .integer(1),
            "kind": .string(kind),
            "causal_parent": .null,
            "data": .object(data)
        ])))
    }
    func shape(index: Int, start: Int, end: Int) -> JSONValue {
        .object([
            "kind": .string("application-transfer"),
            "transfer_id": .string("download"),
            "chunk_index": .integer(Int64(index)),
            "chunk_count": .integer(2),
            "total_bytes": .integer(500_000_000),
            "byte_start": .integer(Int64(start)),
            "byte_end": .integer(Int64(end))
        ])
    }
    var state = SimulationState()
    state.apply(try event("data.flow-offered", id: "old-offer", data: [
        "source": .integer(0), "destination": .integer(2),
        "path": .array([.integer(0), .integer(1), .integer(2)]),
        "shape": shape(index: 0, start: 0, end: 250_000_000)
    ]))
    let reroutedShape = shape(index: 1, start: 250_000_000, end: 500_000_000)
    state.apply(try event("data.flow-offered", id: "new-offer", data: [
        "source": .integer(0), "destination": .integer(2),
        "path": .array([.integer(0), .integer(3), .integer(2)]),
        "shape": reroutedShape
    ]))
    let delivery: [String: JSONValue] = [
        "final": .bool(true),
        "useful_bytes": .integer(250_000_000),
        "shape": reroutedShape
    ]
    state.apply(try event("data.frame-delivered", id: "delivery", data: delivery))
    state.apply(try event("data.frame-delivered", id: "duplicate", data: delivery))
    let transfer = try #require(state.applicationTransfers["download"])
    #expect(transfer.routeLabel == "#0 → #3 → #2")
    #expect(transfer.deliveredBytes == 250_000_000)
    #expect(transfer.progress == 0.5)
}
