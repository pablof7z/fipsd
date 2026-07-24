import Testing
@testable import FIPSDFeature

@Test func recordedDiagnosticsDerivePercentilesPlanesAndZoneCells() throws {
    func event(
        _ id: String, kind: String, at: Int64, data: [String: JSONValue]
    ) throws -> SimulationEvent {
        try #require(SimulationEvent(.object([
            "event_id": .string(id), "virtual_time_ns": .integer(at),
            "ordinal": .integer(at), "kind": .string(kind),
            "causal_parent": .null, "data": .object(data)
        ])))
    }
    let topology = try event("topology", kind: "input.initial-topology", at: 0, data: [
        "nodes": .array([
            .object(["id": .integer(0), "media_zone": .string("zone-a")]),
            .object(["id": .integer(1), "media_zone": .string("zone-b")])
        ])
    ])
    let control = try event("control", kind: "tree-announce.due", at: 100, data: [
        "from": .integer(0), "to": .integer(1), "transport_bytes": .integer(200),
        "queue_occupancy_bytes": .integer(800),
        "deliveries": .array([.object(["deliver_at_ns": .integer(200)])])
    ])
    let payload = try event("payload", kind: "data.frame-due", at: 200, data: [
        "from": .integer(1), "to": .integer(0), "transport_bytes": .integer(600),
        "queue_occupancy_bytes": .integer(5_000),
        "deliveries": .array([.object(["deliver_at_ns": .integer(1_100)])])
    ])

    let result = ArtifactDiagnostics.build([topology, control, payload])

    #expect(result.deliverySamples == 2)
    #expect(result.latencyP50NS == 100)
    #expect(result.latencyP95NS == 900)
    #expect(result.planeLoads.first { $0.label == "payload" }?.count == 600)
    #expect(result.queueHistogram.first { $0.label == "≤1 KiB" }?.count == 1)
    #expect(result.congestion.first { $0.id == "zone-a->zone-b" }?.bytes == 200)
}
