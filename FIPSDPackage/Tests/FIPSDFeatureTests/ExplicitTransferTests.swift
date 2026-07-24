import Foundation
import Testing
@testable import FIPSDFeature

@Test func naturalLanguageDownloadBecomesAnExplicitThreeNodeTransfer() throws {
    let prompt =
        "Three nodes in a chain; the last one is downloading a 500 MB file from the first."
    let intent = try #require(ExplicitTransferIntent.parse(prompt))
    #expect(intent.nodeCount == 3)
    #expect(intent.totalBytes == 500_000_000)
    #expect(!intent.preserveRequestedConnectivity)
    let data = try intent.applying(to: CampaignBuilder.data(for: CampaignConfiguration()))
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let scale = try #require(campaign["scale"] as? [String: Any])
    #expect(scale["nodes"] as? Int == 3)
    let topology = try #require(campaign["topology"] as? [String: Any])
    #expect(topology["generator"] as? String == "explicit")
    #expect(topology["explicit_edges"] as? [[Int]] == [[0, 1], [1, 2]])
    let traffic = try #require(campaign["traffic"] as? [String: Any])
    #expect(traffic["model"] as? String == "explicit-transfers")
    let transfers = try #require(traffic["transfers"] as? [[String: Any]])
    #expect(transfers[0]["source"] as? Int == 0)
    #expect(transfers[0]["destination"] as? Int == 2)
    #expect(transfers[0]["total_bytes"] as? Int == 500_000_000)
    let transports = try #require(campaign["transports"] as? [String: Any])
    #expect(transports["assignment"] as? String == "all-ethernet")
    #expect((campaign["events"] as? [[String: Any]])?.isEmpty == true)
}

@Test func concreteTransferPromptAuthorsWithoutWaitingForAnExternalModel() async throws {
    let (data, provider) = try await PromptAuthor().generate(
        prompt: "Three nodes; the last downloads a 500 MB file from the first.",
        provider: .automatic,
        template: CampaignBuilder.data(for: CampaignConfiguration())
    )
    #expect(provider == .automatic)
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let traffic = try #require(campaign["traffic"] as? [String: Any])
    #expect(traffic["model"] as? String == "explicit-transfers")
}

@Test func requestedMixedConnectivitySurvivesTransferIntentNormalization() throws {
    let prompt =
        "Three nodes: download a 500 MB file through randomized wifi, bluetooth, and Tor links."
    let intent = try #require(ExplicitTransferIntent.parse(prompt))
    #expect(intent.preserveRequestedConnectivity)
    var configuration = CampaignConfiguration()
    configuration.mixedTransports = true
    let data = try intent.applying(to: CampaignBuilder.data(for: configuration))
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let transports = try #require(campaign["transports"] as? [String: Any])
    #expect(transports["assignment"] as? String == "random-mixed")
}

@Test func followUpPromptAmendsTheActiveTransferAtTheCurrentCursor() throws {
    let initial = try #require(ExplicitTransferIntent.parse(
        "Three nodes in a chain download a 500 MB file from the first to the last."
    ))
    let campaign = try initial.applying(
        to: CampaignBuilder.data(for: CampaignConfiguration())
    )
    let followUp = try #require(LiveInterventionIntent.parse(
        "Now make the connection between node b and c spotty, and introduce a new node that becomes root."
    ))
    let amended = try followUp.applying(to: campaign, at: 2_100_000_000)
    let object = try #require(
        JSONSerialization.jsonObject(with: amended) as? [String: Any]
    )
    #expect((object["scale"] as? [String: Any])?["nodes"] as? Int == 4)
    let topology = try #require(object["topology"] as? [String: Any])
    #expect((topology["explicit_edges"] as? [[Int]])?.contains([0, 3]) == true)
    let traffic = try #require(object["traffic"] as? [String: Any])
    let transfers = try #require(traffic["transfers"] as? [[String: Any]])
    #expect(transfers[0]["total_bytes"] as? Int == 500_000_000)
    let events = try #require(object["events"] as? [[String: Any]])
    let link = try #require(events.first { $0["action"] as? String == "set-link-conditions" })
    #expect(link["target"] as? Int == 1)
    #expect(link["at"] as? String == "2100000000ns")
    let parameters = try #require(link["parameters"] as? [String: Any])
    #expect(parameters["bandwidth_bps"] as? Int == 1_000_000)
    #expect(parameters["loss_ppm"] as? Int == 100_000)
    #expect(events.contains { $0["action"] as? String == "introduce-lower-root-node" })
    let root = try #require(
        events.first { $0["action"] as? String == "introduce-lower-root-node" }
    )
    #expect(root["target"] as? Int == 0)
    let identities = try #require(object["identities"] as? [String: Any])
    let arrivals = try #require(identities["arrivals"] as? [String: Any])
    let budget = try #require(arrivals["attacker_budget"] as? [String: Any])
    #expect(budget["identities"] as? Int == 1)
    #expect(budget["operations"] as? Int == 1)
}

@Test func modelAmendmentCanStopFutureArrivalsAndScheduleOnlyFutureEvents() throws {
    var configuration = CampaignConfiguration()
    configuration.nodes = 10
    configuration.arrivals = 20
    let campaign = try CampaignBuilder.data(for: configuration)
    let amendment = try JSONSerialization.data(withJSONObject: [
        "stop_scheduled_arrivals": true,
        "events": [
            [
                "id": "remove-oldest-0", "at": "30s",
                "action": "disappear-node", "target": 0
            ],
            [
                "id": "remove-oldest-1", "at": "35s",
                "action": "disappear-node", "target": 1
            ]
        ]
    ])
    let amended = try CampaignAmendment.applying(
        amendment,
        to: campaign,
        noEarlierThan: 25_100_000_000,
        realizedArrivals: 5
    )
    let object = try #require(
        JSONSerialization.jsonObject(with: amended) as? [String: Any]
    )
    let identities = try #require(object["identities"] as? [String: Any])
    let arrivals = try #require(identities["arrivals"] as? [String: Any])
    #expect(arrivals["count"] as? Int == 5)
    let events = try #require(object["events"] as? [[String: Any]])
    #expect(events.contains { $0["id"] as? String == "remove-oldest-0" })
    #expect(events.contains { $0["id"] as? String == "remove-oldest-1" })
}

@Test func modelAmendmentCanAddAReplacementBridgeBeforeRemovingTheOldOne() throws {
    let initial = try #require(ExplicitTransferIntent.parse(
        "Three nodes download a 500 MB file from the first to the last."
    ))
    let campaign = try initial.applying(
        to: CampaignBuilder.data(for: CampaignConfiguration())
    )
    let amendment = try JSONSerialization.data(withJSONObject: [
        "events": [
            [
                "id": "new-bridge", "at": "2s", "action": "introduce-node",
                "parameters": ["attachments": [0, 2]]
            ],
            [
                "id": "old-bridge-leaves", "at": "12s",
                "action": "disappear-node", "target": 1
            ]
        ]
    ])
    let amended = try CampaignAmendment.applying(
        amendment,
        to: campaign,
        noEarlierThan: 1_500_000_000,
        realizedArrivals: 0,
        attachmentNode: 0
    )
    let object = try #require(
        JSONSerialization.jsonObject(with: amended) as? [String: Any]
    )
    #expect((object["scale"] as? [String: Any])?["nodes"] as? Int == 4)
    let topology = try #require(object["topology"] as? [String: Any])
    let edges = try #require(topology["explicit_edges"] as? [[Int]])
    #expect(edges.contains([0, 3]))
    #expect(edges.contains([2, 3]))
    let events = try #require(object["events"] as? [[String: Any]])
    let arrival = try #require(events.first { $0["action"] as? String == "introduce-node" })
    let parameters = try #require(arrival["parameters"] as? [String: Any])
    #expect(parameters["attachments"] as? [Int] == [0, 2])
}

@Test func modelAmendmentCannotRewriteRenderedHistory() throws {
    let campaign = try CampaignBuilder.data(for: CampaignConfiguration())
    let amendment = try JSONSerialization.data(withJSONObject: [
        "events": [[
            "id": "too-early", "at": "1s",
            "action": "disappear-node", "target": 0
        ]]
    ])
    #expect(throws: CampaignAmendmentError.self) {
        try CampaignAmendment.applying(
            amendment,
            to: campaign,
            noEarlierThan: 2_000_000_000,
            realizedArrivals: 0
        )
    }
}

@Test func liveRunContextIncludesRenderedNodesEdgesAndJoinOrder() throws {
    var state = SimulationState()
    state.nodes[0] = NodeState(
        id: 0, address: "a0", active: true, root: 0, parent: nil, sequence: 1
    )
    state.nodes[1] = NodeState(
        id: 1, address: "b1", active: true, root: 0, parent: 0, sequence: 1
    )
    state.edges[0] = EdgeState(id: 0, from: 0, to: 1, bandwidthBPS: 10_000_000)
    let arrival = try #require(SimulationEvent(.object([
        "event_id": .string("arrival"), "virtual_time_ns": .integer(5_000_000_000),
        "ordinal": .integer(1), "kind": .string("input.node-arrived"),
        "causal_parent": .null, "data": .object(["node": .integer(1)])
    ])))
    let campaign = try CampaignBuilder.data(for: CampaignConfiguration())
    let context = try LiveRunContext.make(
        state: state,
        events: [arrival],
        cursor: 1,
        timeNS: 6_000_000_000,
        campaign: campaign
    )
    #expect(context.realizedArrivals == 1)
    let object = try #require(
        JSONSerialization.jsonObject(with: context.data) as? [String: Any]
    )
    #expect(object["cursor_virtual_time_ns"] as? Int == 6_000_000_000)
    let nodes = try #require(object["nodes_oldest_first"] as? [[String: Any]])
    #expect(nodes.first?["label"] as? String == "a")
    #expect(nodes.last?["joined_at_ns"] as? Int == 5_000_000_000)
    let edges = try #require(object["edges"] as? [[String: Any]])
    #expect(edges.first?["bandwidth_bps"] as? Int == 10_000_000)
}

@Test func explicitTransferStateTracksRouteAndExactByteProgress() throws {
    func event(
        _ kind: String,
        id: String,
        time: Int,
        data: [String: JSONValue]
    ) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string(id),
            "virtual_time_ns": .integer(Int64(time)),
            "ordinal": .integer(Int64(time)),
            "kind": .string(kind),
            "causal_parent": .null,
            "data": .object(data)
        ])))
    }
    let shape: JSONValue = .object([
        "kind": .string("application-transfer"),
        "transfer_id": .string("download"),
        "chunk_index": .integer(0),
        "chunk_count": .integer(2),
        "total_bytes": .integer(500_000_000),
        "byte_start": .integer(0),
        "byte_end": .integer(250_000_000)
    ])
    var state = SimulationState()
    state.apply(try event("data.flow-offered", id: "offer", time: 1, data: [
        "source": .integer(0), "destination": .integer(2),
        "path": .array([.integer(0), .integer(1), .integer(2)]),
        "shape": shape, "status": .string("routed")
    ]))
    state.apply(try event("data.frame-delivered", id: "delivery", time: 2, data: [
        "from": .integer(1), "to": .integer(2), "copy": .integer(0),
        "final": .bool(true), "useful_bytes": .integer(250_000_000),
        "shape": shape
    ]))
    let transfer = try #require(state.applicationTransfers["download"])
    #expect(transfer.routeLabel == "#0 → #1 → #2")
    #expect(transfer.deliveredBytes == 250_000_000)
    #expect(transfer.progress == 0.5)
}

@Test func oneDeliveredChunkDoesNotEraseOtherInFlightChunksOnTheSameHop() throws {
    func event(
        _ kind: String,
        id: String,
        parent: JSONValue = .null,
        data: [String: JSONValue]
    ) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string(id), "virtual_time_ns": .integer(1),
            "ordinal": .integer(1), "kind": .string(kind),
            "causal_parent": parent, "data": .object(data)
        ])))
    }
    let dueData: [String: JSONValue] = [
        "from": .integer(0), "to": .integer(1), "frame_bytes": .integer(1_000_000),
        "transport_bytes": .integer(1_100_000), "queue_occupancy_bytes": .integer(1_000_000),
        "deliveries": .array([.object([
            "deliver_at_ns": .integer(10), "copy": .integer(0)
        ])])
    ]
    var state = SimulationState()
    state.apply(try event("data.frame-due", id: "due-a", data: dueData))
    state.apply(try event("data.frame-due", id: "due-b", data: dueData))
    state.apply(try event(
        "data.frame-delivered",
        id: "delivered-a",
        parent: .string("due-a"),
        data: [
            "from": .integer(0), "to": .integer(1), "copy": .integer(0),
            "final": .bool(false), "useful_bytes": .integer(0)
        ]
    ))
    #expect(state.transmissions["due-a:0"] == nil)
    #expect(state.transmissions["due-b:0"] != nil)
}
