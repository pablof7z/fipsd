import Foundation
import Testing
@testable import FIPSDFeature

@Test func configuredCampaignUsesAuthoringDurationStrings() throws {
    let data = try CampaignBuilder.data(for: CampaignConfiguration())
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let identities = try #require(campaign["identities"] as? [String: Any])
    let arrivals = try #require(identities["arrivals"] as? [String: Any])
    let schedule = try #require(arrivals["schedule"] as? [String: Any])
    #expect(schedule["interval"] as? String == "1000000000ns")
    #expect(arrivals["attachment"] as? String == "random")
    let transports = try #require(campaign["transports"] as? [String: Any])
    #expect(transports["assignment"] as? String == "random-mixed")
    let profiles = try #require(transports["profiles"] as? [[String: Any]])
    #expect(profiles.count == 4)
    #expect(profiles.contains { $0["type"] as? String == "ble" })
    #expect(profiles.allSatisfy { ($0["bandwidth_bps"] as? Int ?? 0) > 0 })
    #expect(profiles.allSatisfy { $0["jitter"] as? String != nil })
    let links = try #require(campaign["links"] as? [String: Any])
    #expect(links["jitter"] as? String == "10000000ns")
    let traffic = try #require(campaign["traffic"] as? [String: Any])
    #expect(traffic["model"] as? String == "uniform-random")
    #expect(traffic["payload_bytes"] as? Int == 512)

    var bloomConfiguration = CampaignConfiguration()
    bloomConfiguration.bloomEnabled = true
    let bloomData = try CampaignBuilder.data(for: bloomConfiguration)
    let bloomCampaign = try #require(JSONSerialization.jsonObject(with: bloomData) as? [String: Any])
    let instrumentation = try #require(bloomCampaign["instrumentation"] as? [String: Any])
    #expect(instrumentation["quiescence_markers"] as? [String] == ["root", "tree", "bloom"])

    var recoveryConfiguration = CampaignConfiguration()
    recoveryConfiguration.bloomEnabled = true
    recoveryConfiguration.lookupRecoveryEnabled = true
    let recoveryData = try CampaignBuilder.data(for: recoveryConfiguration)
    let recoveryCampaign = try #require(JSONSerialization.jsonObject(with: recoveryData) as? [String: Any])
    let recoveryInstrumentation = try #require(recoveryCampaign["instrumentation"] as? [String: Any])
    #expect(recoveryInstrumentation["quiescence_markers"] as? [String] == ["root", "tree", "bloom", "lookup"])

    var explicitConfiguration = CampaignConfiguration()
    explicitConfiguration.nodes = 3
    explicitConfiguration.arrivals = 0
    explicitConfiguration.topology = "explicit"
    explicitConfiguration.explicitEdges = [ManualEdge(0, 1), ManualEdge(2, 1)]
    let explicitData = try CampaignBuilder.data(for: explicitConfiguration)
    let explicitCampaign = try #require(JSONSerialization.jsonObject(with: explicitData) as? [String: Any])
    let explicitTopology = try #require(explicitCampaign["topology"] as? [String: Any])
    #expect(explicitTopology["generator"] as? String == "explicit")
    #expect(explicitTopology["explicit_edges"] as? [[Int]] == [[0, 1], [1, 2]])

    var constrained = CampaignConfiguration()
    constrained.heterogeneousResources = true
    constrained.cpuUnitsPerMS = 250
    constrained.memoryMB = 64
    constrained.nodeQueueKB = 32
    constrained.tableEntries = 128
    constrained.coordinateCacheEntries = 8
    constrained.lookupTTL = 12
    let constrainedData = try CampaignBuilder.data(for: constrained)
    let constrainedCampaign = try #require(JSONSerialization.jsonObject(with: constrainedData) as? [String: Any])
    let resources = try #require(constrainedCampaign["resources"] as? [String: Any])
    #expect(resources["assignment"] as? String == "heterogeneous")
    let constrainedProfiles = try #require(resources["node_profiles"] as? [[String: Any]])
    #expect(constrainedProfiles[0]["cpu_units"] as? Int == 250)
    #expect(constrainedProfiles[0]["memory_bytes"] as? Int == 64 * 1_048_576)
    let protocolObject = try #require(constrainedCampaign["protocol"] as? [String: Any])
    let parameters = try #require(protocolObject["parameters"] as? [String: Any])
    #expect(parameters["coord_cache_entries"] as? Int == 8)
    #expect(parameters["lookup_ttl"] as? Int == 12)

    let cohortData = try CampaignBuilder.cohortData(for: constrained, nodes: 1_000_000_000)
    let cohortCampaign = try #require(JSONSerialization.jsonObject(with: cohortData) as? [String: Any])
    let cohortEngine = try #require(cohortCampaign["engine"] as? [String: Any])
    let cohortScale = try #require(cohortCampaign["scale"] as? [String: Any])
    let cohortFidelity = try #require(cohortCampaign["fidelity"] as? [String: Any])
    #expect(cohortEngine["modes"] as? String == "cohort-analytical")
    #expect(cohortScale["nodes"] as? Int == 1_000_000_000)
    #expect(cohortFidelity["protocol"] as? String == "cohort")

    let searchData = try CampaignBuilder.searchData(for: constrained)
    let searchCampaign = try #require(JSONSerialization.jsonObject(with: searchData) as? [String: Any])
    let searchScale = try #require(searchCampaign["scale"] as? [String: Any])
    let nodeAxis = try #require(searchScale["nodes"] as? [String: Any])
    #expect((nodeAxis["values"] as? [Int])?.count == 2)
    let searchTraffic = try #require(searchCampaign["traffic"] as? [String: Any])
    let trafficAxis = try #require(searchTraffic["model"] as? [String: Any])
    #expect((trafficAxis["values"] as? [String])?.contains("session-churn") == true)
}

@Test func configuredInterventionsAreDurableCampaignEvents() throws {
    var configuration = CampaignConfiguration()
    configuration.manualRootTimes = [2.25]
    configuration.networkEvents = [NetworkIntervention(
        atSeconds: 3, action: "partition-network", nodes: [4]
    )]
    configuration.linkEvents = [LinkIntervention(
        atSeconds: 4, action: "set-link-conditions", edge: 2,
        bandwidthMbps: 1, latencyMilliseconds: 200, jitterMilliseconds: 50,
        lossPPM: 500, mtuBytes: 1_280
    )]
    configuration.sessionRekeyTimes = [3.5]
    let data = try CampaignBuilder.data(for: configuration)
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let events = try #require(campaign["events"] as? [[String: Any]])
    #expect(events.contains { $0["action"] as? String == "introduce-lower-root-node" })
    #expect(events.contains { $0["action"] as? String == "partition-network" })
    #expect(events.contains { $0["action"] as? String == "synchronized-session-rekey" })
    let link = try #require(events.first { $0["action"] as? String == "set-link-conditions" })
    let parameters = try #require(link["parameters"] as? [String: Any])
    #expect(parameters["bandwidth_bps"] as? Int == 1_000_000)
    #expect(parameters["jitter"] as? String == "50000000ns")
    let identities = try #require(campaign["identities"] as? [String: Any])
    let arrivals = try #require(identities["arrivals"] as? [String: Any])
    let budget = try #require(arrivals["attacker_budget"] as? [String: Any])
    #expect(budget["operations"] as? Int == configuration.arrivals + 1)
}

@MainActor
@Test func tenThousandPresetSelectsCohortRepresentation() {
    let model = WorkbenchModel()
    model.loadTenThousandPreset()
    #expect(model.configuration.nodes == 10_000)
    #expect(model.visualizationMode == .cohorts)
}

@MainActor
@Test func renderedTopologyCanBecomeAnEditableCampaignGraph() throws {
    let topology = try #require(SimulationEvent(.object([
        "event_id": .string("topology"), "virtual_time_ns": .integer(0),
        "ordinal": .integer(0), "kind": .string("input.initial-topology"),
        "causal_parent": .null, "data": .object([
            "nodes": .array((0..<3).map { id in .object([
                "id": .integer(Int64(id)), "address": .string("8\(id)"),
                "active": .bool(true), "root": .integer(Int64(id)),
                "parent": .null, "sequence": .integer(1)
            ]) }),
            "edges": .array([
                .object(["id": .integer(0), "from": .integer(0), "to": .integer(1)]),
                .object(["id": .integer(1), "from": .integer(1), "to": .integer(2)])
            ])
        ])
    ])))
    let model = WorkbenchModel()
    model.state.apply(topology)
    model.captureRenderedTopology()
    #expect(model.configuration.topology == "explicit")
    #expect(model.configuration.nodes == 3)
    #expect(model.configuration.explicitEdges == [ManualEdge(0, 1), ManualEdge(1, 2)])
}

@MainActor
@Test func savedArtifactLoadsIntoTheAnimatedTimeline() throws {
    let artifact: [String: Any] = [
        "manifest": [
            "run_id": "run-loaded", "artifact_id": "artifact-loaded",
            "fidelity": ["scale": "individual", "protocol": "semantic-exact", "wire": "modeled"]
        ],
        "event_trace": [[
            "event_id": "event-1", "virtual_time_ns": 0, "ordinal": 0,
            "kind": "input.initial-topology", "causal_parent": NSNull(),
            "data": [
                "nodes": [[
                    "id": 0, "address": "80", "active": true,
                    "root": 0, "parent": NSNull(), "sequence": 1
                ]],
                "edges": []
            ]
        ], [
            "event_id": "event-2", "virtual_time_ns": 100, "ordinal": 1,
            "kind": "tree-announce.due", "causal_parent": "event-1",
            "data": [
                "from": 0, "to": 1, "transport_bytes": 200,
                "queue_occupancy_bytes": 300
            ]
        ]],
        "causal_ledger": [[
            "causal_id": "input:1", "stage": "queued", "count": 200
        ]],
        "metric_series": [[
            "name": "tree.maximum-depth", "unit": "edges",
            "points": [["virtual_time_ns": 100, "value": "3"]]
        ]],
        "assertion_results": [["id": "determinism", "outcome": "pass"]],
        "reports": [["final_root": "80", "quiescence_ns": 0]]
    ]
    let data = try JSONSerialization.data(withJSONObject: artifact)
    let model = WorkbenchModel()
    try model.loadArtifactData(data)
    model.seek(to: 0)
    #expect(model.events.count == 2)
    #expect(model.state.nodes.count == 1)
    #expect(model.summary.runID == "run-loaded")
    #expect(model.summary.outcome == "pass")
    #expect(model.status.contains("Loaded saved artifact"))
    #expect(model.analysis.eventCount == 2)
    #expect(model.analysis.stages.first?.stage == "queued")
    #expect(model.analysis.topEdges.first?.bytes == 200)
    #expect(model.analysis.longestCausalChain == 2)
}

@Test func lookupAndSessionFramesAnimateAsDistinctPlanes() throws {
    func event(_ kind: String, message: String, ordinal: Int) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string("event-\(ordinal)"),
            "virtual_time_ns": .integer(Int64(ordinal * 100)),
            "ordinal": .integer(Int64(ordinal)), "kind": .string(kind),
            "causal_parent": .null, "data": .object([
                "from": .integer(1), "to": .integer(2), "message": .string(message),
                "frame_bytes": .integer(145), "transport_bytes": .integer(177),
                "queue_occupancy_bytes": .integer(177),
                "deliveries": .array([.object([
                    "deliver_at_ns": .integer(Int64((ordinal + 1) * 100)),
                    "copy": .integer(0)
                ])])
            ])
        ])))
    }
    var state = SimulationState()
    state.apply(try event("lookup.frame-due", message: "lookup-response", ordinal: 1))
    state.apply(try event("session.frame-due", message: "session-setup", ordinal: 2))
    #expect(Set(state.transmissions.values.map(\.plane)) == Set(["lookup", "session"]))
}

@Test func bloomFramesAnimateSeparatelyFromTreeAndPayload() throws {
    let due = try #require(SimulationEvent(.object([
        "event_id": .string("bloom-due"), "virtual_time_ns": .integer(100),
        "ordinal": .integer(1), "kind": .string("bloom.filter-due"),
        "causal_parent": .null, "data": .object([
            "from": .integer(2), "to": .integer(3), "frame_bytes": .integer(1071),
            "transport_bytes": .integer(1099), "queue_occupancy_bytes": .integer(1099),
            "deliveries": .array([.object([
                "deliver_at_ns": .integer(200), "copy": .integer(0)
            ])])
        ])
    ])))
    let delivered = try #require(SimulationEvent(.object([
        "event_id": .string("bloom-delivered"), "virtual_time_ns": .integer(200),
        "ordinal": .integer(2), "kind": .string("bloom.filter-delivered"),
        "causal_parent": .string("bloom-due"), "data": .object([
            "from": .integer(2), "to": .integer(3), "copy": .integer(0)
        ])
    ])))
    var state = SimulationState()
    state.apply(due)
    #expect(state.transmissions.values.first?.plane == "bloom")
    state.apply(delivered)
    #expect(state.bloomDelivered == 1)
    #expect(state.transmissions.isEmpty)
}

@Test func routedPayloadHopsAnimateAndCreditUsefulDelivery() throws {
    func event(_ kind: String, data: [String: JSONValue], ordinal: Int) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string("event-\(ordinal)"),
            "virtual_time_ns": .integer(Int64(ordinal * 100)),
            "ordinal": .integer(Int64(ordinal)),
            "kind": .string(kind), "causal_parent": .null, "data": .object(data)
        ])))
    }
    var state = SimulationState()
    state.apply(try event("data.frame-due", data: [
        "from": .integer(1), "to": .integer(2), "frame_bytes": .integer(618),
        "transport_bytes": .integer(650), "queue_occupancy_bytes": .integer(650),
        "deliveries": .array([.object(["deliver_at_ns": .integer(200), "copy": .integer(0)])])
    ], ordinal: 1))
    #expect(state.transmissions.values.first?.plane == "data")
    state.apply(try event("data.frame-delivered", data: [
        "from": .integer(1), "to": .integer(2), "copy": .integer(0),
        "final": .bool(true), "useful_bytes": .integer(512)
    ], ordinal: 2))
    #expect(state.flowsDelivered == 1)
    #expect(state.usefulBytesDelivered == 512)
    #expect(state.transmissions.isEmpty)
}

@Test func topologyAndDeliveriesProjectIntoRenderableState() throws {
    let topology: JSONValue = .object([
        "event_id": .string("event-1"),
        "virtual_time_ns": .integer(0),
        "ordinal": .integer(0),
        "kind": .string("input.initial-topology"),
        "causal_parent": .null,
        "data": .object([
            "nodes": .array([
                .object([
                    "id": .integer(0), "address": .string("80"),
                    "active": .bool(true), "root": .integer(0),
                    "parent": .null, "sequence": .integer(1)
                ]),
                .object([
                    "id": .integer(1), "address": .string("81"),
                    "active": .bool(true), "root": .integer(1),
                    "parent": .null, "sequence": .integer(1)
                ])
            ]),
            "edges": .array([.object([
                "id": .integer(0), "from": .integer(0), "to": .integer(1)
            ])])
        ])
    ])
    var state = SimulationState()
    state.apply(try #require(SimulationEvent(topology)))
    #expect(state.nodes.count == 2)
    #expect(state.edges.count == 1)
}


@Test func partitionAndLinkUpdatesProjectIntoEdgeState() throws {
    func event(_ kind: String, data: [String: JSONValue], ordinal: Int) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string("event-\(ordinal)"),
            "virtual_time_ns": .integer(Int64(ordinal * 100)),
            "ordinal": .integer(Int64(ordinal)), "kind": .string(kind),
            "causal_parent": .null, "data": .object(data)
        ])))
    }
    var state = SimulationState()
    state.apply(try event("input.initial-topology", data: [
        "nodes": .array([]),
        "edges": .array([.object([
            "id": .integer(0), "from": .integer(0), "to": .integer(1),
            "active": .bool(true), "bandwidth_bps": .integer(100_000_000),
            "latency_ns": .integer(8_000_000), "loss_ppm": .integer(0),
            "mtu_bytes": .integer(1_500), "queue_bytes": .integer(1_024)
        ])])
    ], ordinal: 0))
    state.apply(try event("input.network-partitioned", data: [
        "changed_edges": .array([.object([
            "id": .integer(0), "from": .integer(0), "to": .integer(1)
        ])])
    ], ordinal: 1))
    #expect(state.edges[0]?.active == false)
    state.apply(try event("input.link-conditions-changed", data: [
        "edge": .integer(0), "after": .object([
            "bandwidth_bps": .integer(1_000_000), "latency_ns": .integer(200_000_000),
            "loss_ppm": .integer(500), "mtu_bytes": .integer(1_280),
            "queue_bytes": .integer(2_048)
        ])
    ], ordinal: 2))
    #expect(state.edges[0]?.bandwidthBPS == 1_000_000)
    #expect(state.edges[0]?.latencyNS == 200_000_000)
}

@Test func variantComparisonReportsCompatibleMetricAndTraceDeltas() {
    let left = ArtifactAnalysis(
        representedNodes: 10, fidelity: "exact",
        metrics: [AnalysisMetric(name: "control-bytes", unit: "bytes", value: "100")],
        stages: [], topEdges: [], rootImpacts: [], longestCausalChain: 2,
        ledgerEntries: 3, eventCount: 4
    )
    let right = ArtifactAnalysis(
        representedNodes: 10, fidelity: "exact",
        metrics: [AnalysisMetric(name: "control-bytes", unit: "bytes", value: "80")],
        stages: [], topEdges: [], rootImpacts: [], longestCausalChain: 2,
        ledgerEntries: 3, eventCount: 4
    )
    let comparison = VariantComparison.compare(
        baseline: VariantRunEvidence(analysis: left, events: []),
        candidate: VariantRunEvidence(analysis: right, events: []),
        baselineDebounceMS: 500, candidateDebounceMS: 2_000
    )
    #expect(comparison.compatible)
    #expect(comparison.deltas.first { $0.name == "control-bytes" }?.delta == -20)
    #expect(comparison.firstDivergence == nil)
}
