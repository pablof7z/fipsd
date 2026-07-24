import Testing
@testable import FIPSDFeature

private func diagnosticEvent(
    _ id: String,
    kind: String,
    at: Int64,
    parent: String?,
    data: [String: JSONValue]
) throws -> SimulationEvent {
    try #require(SimulationEvent(.object([
        "event_id": .string(id),
        "virtual_time_ns": .integer(at),
        "ordinal": .integer(at),
        "kind": .string(kind),
        "causal_parent": parent.map(JSONValue.string) ?? .null,
        "data": .object(data)
    ])))
}

@Test func bloomFPRDistributionUsesRecordedDeliverySamples() throws {
    let events = try [10, 30, 20].enumerated().map { index, value in
        try diagnosticEvent(
            "bloom-\(index)",
            kind: "bloom.filter-delivered",
            at: Int64(index),
            parent: nil,
            data: ["fpr_ppb": .integer(Int64(value))]
        )
    }
    let result = ArtifactDiagnostics.build(events)
    #expect(result.bloomFPRSamples == 3)
    #expect(result.bloomFPRP50PPB == 20)
    #expect(result.bloomFPRP95PPB == 30)
    #expect(result.bloomFPRP99PPB == 30)
}

@Test func causalFlamesAndAnomalySampleComeFromRecordedLineageAndLoad() throws {
    let input = try diagnosticEvent(
        "input", kind: "input.authenticated-sybil-arrived", at: 0,
        parent: nil, data: [:]
    )
    let due = try diagnosticEvent(
        "due", kind: "tree-announce.due", at: 1,
        parent: "input",
        data: [
            "from": .integer(4), "to": .integer(9),
            "transport_bytes": .integer(500), "queue_occupancy_bytes": .integer(100)
        ]
    )
    let delivered = try diagnosticEvent(
        "delivered", kind: "tree-announce.delivered", at: 2,
        parent: "due", data: ["from": .integer(4), "to": .integer(9)]
    )
    let values = [input, due, delivered].map { event in
        JSONValue.object([
            "event_id": .string(event.id),
            "virtual_time_ns": .integer(Int64(event.timeNS)),
            "ordinal": .integer(Int64(event.ordinal)),
            "kind": .string(event.kind),
            "causal_parent": event.causalParent.map(JSONValue.string) ?? .null,
            "data": .object(event.data)
        ])
    }
    let analysis = ArtifactAnalysis.parse([
        "event_trace": .array(values),
        "causal_ledger": .array([]),
        "metric_series": .array([])
    ])
    #expect(analysis.causalFlames.first?.eventCount == 3)
    #expect(analysis.causalFlames.first?.slices.first { $0.label == "tree" }?.count == 2)
    #expect(analysis.anomalyNodeIDs == Set([4, 9]))
}
