import Foundation
import Testing
@testable import FIPSDFeature

@Test func billionSensitivityRetainsBoundsAndExactSampleBoundary() throws {
    func estimate(_ value: String) -> [String: Any] {
        [
            "value": value, "lower": "90", "upper": "110", "unit": "bytes",
            "method": "bounded-model/v1", "uncertainty": "deterministic bounds"
        ]
    }
    let document: [String: Any] = [
        "kind": "honest-billion-node-root-ratchet/v1alpha1",
        "represented_nodes": 1_000_000_000,
        "representation_claim": "cohorts plus one exact sample",
        "headline_warning": "aggregate outside the sample",
        "minimum_control_bytes": "100",
        "maximum_control_bytes": "400",
        "resource_budget": ["maximum_exact_nodes": 16, "maximum_allocated_cohorts": 64],
        "scenarios": [[
            "topology": "chain", "cadence_ns": 500_000_000,
            "variant": "fips-80c956a-baseline",
            "control_bytes": estimate("400"), "peak_queue_bytes": estimate("100"),
            "bloom_fpr_ppb": estimate("50"), "maximum_depth": estimate("10")
        ]]
    ]
    let data = try JSONSerialization.data(withJSONObject: document)
    let root = try #require(JSONDecoder().decode(JSONValue.self, from: data).object)
    let sensitivity = try #require(ScaleSensitivity.parse(root))
    #expect(sensitivity.representedNodes == 1_000_000_000)
    #expect(sensitivity.exactSampleNodes == 16)
    #expect(sensitivity.maximumCohorts == 64)
    #expect(sensitivity.controlSpan == 4)
    #expect(sensitivity.scenarios.first?.controlBytes.uncertainty == "deterministic bounds")
}

@Test func authoredMediaZonesPartitionEveryNodeAndPublishCapacity() throws {
    var configuration = CampaignConfiguration()
    configuration.nodes = 8
    configuration.arrivals = 0
    configuration.mediaZonesEnabled = true
    configuration.mediaZoneCount = 3
    configuration.mediaZoneBandwidthMbps = 7
    let data = try CampaignBuilder.data(for: configuration)
    let campaign = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let topology = try #require(campaign["topology"] as? [String: Any])
    let zones = try #require(topology["media_zones"] as? [[String: Any]])
    #expect(zones.count == 3)
    let members = zones.flatMap { $0["nodes"] as? [Int] ?? [] }.sorted()
    #expect(members == Array(0..<8))
    #expect(zones.allSatisfy { $0["bandwidth_bps"] as? Int == 7_000_000 })
}

@Test func boundedSearchPreservesAuthoredMediaZones() throws {
    var configuration = CampaignConfiguration()
    configuration.nodes = 40
    configuration.mediaZonesEnabled = true
    configuration.mediaZoneCount = 3

    let data = try CampaignBuilder.searchData(for: configuration)
    let campaign = try #require(
        JSONSerialization.jsonObject(with: data) as? [String: Any]
    )
    let scale = try #require(campaign["scale"] as? [String: Any])
    let nodes = try #require(scale["nodes"] as? [String: Any])
    #expect(nodes["values"] as? [Int] == [24])

    let topology = try #require(campaign["topology"] as? [String: Any])
    let zones = try #require(topology["media_zones"] as? [[String: Any]])
    #expect(zones.count == 3)
    let members = zones.flatMap { $0["nodes"] as? [Int] ?? [] }.sorted()
    #expect(members == Array(0..<24))
}
