struct CampaignConfiguration: Equatable, Sendable {
    var nodes = 1_000
    var arrivals = 8
    var intervalSeconds = 1.0
    var topology = "random-regular"
    var averageDegree = 4
    var attachment = "random"
    var explicitEdges: [ManualEdge] = []
    var mediaZonesEnabled = false
    var mediaZoneCount = 3
    var mediaZoneBandwidthMbps = 20
    var mediaZoneLatencyMilliseconds = 5.0
    var mediaZoneLossPPM = 1_000
    var mediaZoneMTUBytes = 1_500
    var mediaZoneQueueKB = 512
    var latencyMilliseconds = 20.0
    var jitterMilliseconds = 10.0
    var lossPPM = 0
    var bandwidthMbps = 10_000
    var mtuBytes = 1_500
    var debounceMilliseconds = 500.0
    var bloomEnabled = false
    var lookupRecoveryEnabled = false
    var lifecycleEvents: [LifecycleEvent] = []
    var manualRootTimes: [Double] = []
    var networkEvents: [NetworkIntervention] = []
    var linkEvents: [LinkIntervention] = []
    var sessionRekeyTimes: [Double] = []
    var lookupStorms: [LookupStormIntervention] = []
    var transportClassEvents: [TransportClassIntervention] = []
    var parentEvents: [ParentIntervention] = []
    var sybilEvents: [SybilIntervention] = []
    var interventionTransportProfile = "tor"
    var parentOscillationCycles = 4
    var parentOscillationIntervalMilliseconds = 250
    var parentHysteresisPercent = 0
    var parentHoldDownMilliseconds = 0
    var sybilCount = 10
    var sybilIntervalMilliseconds = 100
    var sybilAttachment = "hub"
    var sybilRootGrinding = false
    var mixedTransports = true
    var wifiWeight = 50
    var wifiMbps = 100
    var bluetoothWeight = 15
    var bluetoothMbps = 1
    var torWeight = 15
    var torMbps = 10
    var ethernetWeight = 20
    var ethernetMbps = 1_000
    var trafficEnabled = true
    var trafficModel = "uniform-random"
    var trafficFlows = 200
    var trafficPayloadBytes = 512
    var trafficRateMbps = 5
    var trafficSegmentsPerStream = 32
    var trafficBurstSize = 16
    var trafficBurstIntervalMilliseconds = 250
    var heterogeneousResources = false
    var cpuUnitsPerMS = 1_000
    var memoryMB = 1_024
    var nodeQueueKB = 1_024
    var tableEntries = 100_000
    var coordinateCacheEntries = 64
    var lookupTTL = 64
    var lookupAttempts = 3
    var lookupStormCount = 100
}

struct LifecycleEvent: Equatable, Sendable {
    let atSeconds: Double
    let action: String
    let node: Int
}

struct ManualEdge: Equatable, Hashable, Sendable {
    let from: Int
    let to: Int

    init(_ first: Int, _ second: Int) {
        from = min(first, second)
        to = max(first, second)
    }
}
