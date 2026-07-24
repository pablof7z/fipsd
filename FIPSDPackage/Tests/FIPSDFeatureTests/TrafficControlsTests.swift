import Foundation
import Testing
@testable import FIPSDFeature

@Test func temporalTrafficControlsProduceExecutableCampaignParameters() throws {
    var persistent = CampaignConfiguration()
    persistent.trafficModel = "persistent-streams"
    persistent.trafficFlows = 3
    persistent.trafficSegmentsPerStream = 7
    persistent.trafficPayloadBytes = 1_200
    let persistentData = try CampaignBuilder.data(for: persistent)
    let persistentCampaign = try #require(
        JSONSerialization.jsonObject(with: persistentData) as? [String: Any]
    )
    let persistentTraffic = try #require(
        persistentCampaign["traffic"] as? [String: Any]
    )
    let persistentParameters = try #require(
        persistentTraffic["parameters"] as? [String: Any]
    )
    #expect(persistentTraffic["model"] as? String == "persistent-streams")
    #expect(persistentParameters["flow_count"] as? Int == 3)
    #expect(persistentParameters["segments_per_stream"] as? Int == 7)

    var bursty = CampaignConfiguration()
    bursty.trafficModel = "bursty"
    bursty.trafficFlows = 19
    bursty.trafficBurstSize = 8
    bursty.trafficBurstIntervalMilliseconds = 125
    let burstData = try CampaignBuilder.data(for: bursty)
    let burstCampaign = try #require(
        JSONSerialization.jsonObject(with: burstData) as? [String: Any]
    )
    let burstTraffic = try #require(burstCampaign["traffic"] as? [String: Any])
    let burstParameters = try #require(burstTraffic["parameters"] as? [String: Any])
    #expect(burstTraffic["model"] as? String == "bursty")
    #expect(burstParameters["burst_size"] as? Int == 8)
    #expect(burstParameters["burst_interval_ns"] as? Int == 125_000_000)
}
