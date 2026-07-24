import Foundation

extension CampaignBuilder {
    static func tinyExplorationData(
        for source: CampaignConfiguration, maximumNodes: Int
    ) throws -> Data {
        var tiny = source
        tiny.nodes = min(max(2, source.nodes), min(maximumNodes, 8))
        tiny.arrivals = 0
        tiny.trafficFlows = min(source.trafficFlows, 16)
        tiny.mediaZoneCount = min(source.mediaZoneCount, tiny.nodes)
        let authored = tiny.lifecycleEvents.count + tiny.manualRootTimes.count
            + tiny.networkEvents.count + tiny.linkEvents.count
        if authored == 0 {
            let node = min(1, tiny.nodes - 1)
            tiny.lifecycleEvents = [
                LifecycleEvent(atSeconds: 1, action: "disappear-node", node: node),
                LifecycleEvent(atSeconds: 2, action: "reappear-node", node: node)
            ]
        }
        return try data(for: tiny)
    }
}
