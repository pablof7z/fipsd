import Foundation

extension WorkbenchModel {
    func applyControlParameters(_ values: [String: JSONValue]) throws {
        var candidate = configuration
        for (name, value) in values {
            switch name {
            case "nodes": candidate.nodes = try integer(value, name)
            case "arrivals": candidate.arrivals = try integer(value, name)
            case "interval_seconds": candidate.intervalSeconds = try decimal(value, name)
            case "topology": candidate.topology = try text(value, name)
            case "average_degree": candidate.averageDegree = try integer(value, name)
            case "attachment": candidate.attachment = try text(value, name)
            case "bandwidth_mbps": candidate.bandwidthMbps = try integer(value, name)
            case "latency_ms": candidate.latencyMilliseconds = try decimal(value, name)
            case "jitter_ms": candidate.jitterMilliseconds = try decimal(value, name)
            case "loss_ppm": candidate.lossPPM = try integer(value, name)
            case "mtu_bytes": candidate.mtuBytes = try integer(value, name)
            case "debounce_ms": candidate.debounceMilliseconds = try decimal(value, name)
            case "bloom_enabled": candidate.bloomEnabled = try boolean(value, name)
            case "lookup_recovery_enabled":
                candidate.lookupRecoveryEnabled = try boolean(value, name)
            case "mixed_transports":
                candidate.mixedTransports = try boolean(value, name)
            case "wifi_weight": candidate.wifiWeight = try integer(value, name)
            case "wifi_mbps": candidate.wifiMbps = try integer(value, name)
            case "bluetooth_weight":
                candidate.bluetoothWeight = try integer(value, name)
            case "bluetooth_mbps":
                candidate.bluetoothMbps = try integer(value, name)
            case "tor_weight": candidate.torWeight = try integer(value, name)
            case "tor_mbps": candidate.torMbps = try integer(value, name)
            case "ethernet_weight":
                candidate.ethernetWeight = try integer(value, name)
            case "ethernet_mbps":
                candidate.ethernetMbps = try integer(value, name)
            case "traffic_enabled":
                candidate.trafficEnabled = try boolean(value, name)
            case "traffic_model": candidate.trafficModel = try text(value, name)
            case "traffic_flows": candidate.trafficFlows = try integer(value, name)
            case "traffic_payload_bytes":
                candidate.trafficPayloadBytes = try integer(value, name)
            case "traffic_rate_mbps":
                candidate.trafficRateMbps = try integer(value, name)
            case "traffic_segments_per_stream":
                candidate.trafficSegmentsPerStream = try integer(value, name)
            case "traffic_burst_size":
                candidate.trafficBurstSize = try integer(value, name)
            case "traffic_burst_interval_ms":
                candidate.trafficBurstIntervalMilliseconds = try integer(value, name)
            case "heterogeneous_resources":
                candidate.heterogeneousResources = try boolean(value, name)
            case "cpu_units_per_ms":
                candidate.cpuUnitsPerMS = try integer(value, name)
            case "memory_mb": candidate.memoryMB = try integer(value, name)
            case "node_queue_kb": candidate.nodeQueueKB = try integer(value, name)
            case "table_entries": candidate.tableEntries = try integer(value, name)
            case "coordinate_cache_entries":
                candidate.coordinateCacheEntries = try integer(value, name)
            case "lookup_ttl": candidate.lookupTTL = try integer(value, name)
            case "lookup_attempts": candidate.lookupAttempts = try integer(value, name)
            case "lookup_storm_count":
                candidate.lookupStormCount = try integer(value, name)
            default:
                throw AppControlError.invalidArgument("parameters.\(name)")
            }
        }
        _ = try CampaignBuilder.data(for: candidate)
        configuration = candidate
        status = "MCP updated \(values.count) configured parameter(s)."
    }

    func controlConfiguration() -> JSONValue {
        .object([
            "nodes": .integer(Int64(configuration.nodes)),
            "arrivals": .integer(Int64(configuration.arrivals)),
            "interval_seconds": .number(configuration.intervalSeconds),
            "topology": .string(configuration.topology),
            "average_degree": .integer(Int64(configuration.averageDegree)),
            "attachment": .string(configuration.attachment),
            "bandwidth_mbps": .integer(Int64(configuration.bandwidthMbps)),
            "latency_ms": .number(configuration.latencyMilliseconds),
            "jitter_ms": .number(configuration.jitterMilliseconds),
            "loss_ppm": .integer(Int64(configuration.lossPPM)),
            "mtu_bytes": .integer(Int64(configuration.mtuBytes)),
            "debounce_ms": .number(configuration.debounceMilliseconds),
            "bloom_enabled": .bool(configuration.bloomEnabled),
            "lookup_recovery_enabled": .bool(configuration.lookupRecoveryEnabled),
            "mixed_transports": .bool(configuration.mixedTransports),
            "wifi_weight": .integer(Int64(configuration.wifiWeight)),
            "wifi_mbps": .integer(Int64(configuration.wifiMbps)),
            "bluetooth_weight": .integer(Int64(configuration.bluetoothWeight)),
            "bluetooth_mbps": .integer(Int64(configuration.bluetoothMbps)),
            "tor_weight": .integer(Int64(configuration.torWeight)),
            "tor_mbps": .integer(Int64(configuration.torMbps)),
            "ethernet_weight": .integer(Int64(configuration.ethernetWeight)),
            "ethernet_mbps": .integer(Int64(configuration.ethernetMbps)),
            "traffic_enabled": .bool(configuration.trafficEnabled),
            "traffic_model": .string(configuration.trafficModel),
            "traffic_flows": .integer(Int64(configuration.trafficFlows)),
            "traffic_payload_bytes": .integer(Int64(configuration.trafficPayloadBytes)),
            "traffic_rate_mbps": .integer(Int64(configuration.trafficRateMbps)),
            "heterogeneous_resources": .bool(configuration.heterogeneousResources),
            "cpu_units_per_ms": .integer(Int64(configuration.cpuUnitsPerMS)),
            "memory_mb": .integer(Int64(configuration.memoryMB)),
            "node_queue_kb": .integer(Int64(configuration.nodeQueueKB)),
            "table_entries": .integer(Int64(configuration.tableEntries)),
            "coordinate_cache_entries": .integer(
                Int64(configuration.coordinateCacheEntries)
            ),
            "lookup_ttl": .integer(Int64(configuration.lookupTTL)),
            "lookup_attempts": .integer(Int64(configuration.lookupAttempts)),
            "lookup_storm_count": .integer(Int64(configuration.lookupStormCount))
        ])
    }

    private func integer(_ value: JSONValue, _ name: String) throws -> Int {
        guard let result = value.int else {
            throw AppControlError.invalidArgument("parameters.\(name)")
        }
        return result
    }

    private func decimal(_ value: JSONValue, _ name: String) throws -> Double {
        guard let result = value.double, result.isFinite else {
            throw AppControlError.invalidArgument("parameters.\(name)")
        }
        return result
    }

    private func text(_ value: JSONValue, _ name: String) throws -> String {
        guard let result = value.string else {
            throw AppControlError.invalidArgument("parameters.\(name)")
        }
        return result
    }

    private func boolean(_ value: JSONValue, _ name: String) throws -> Bool {
        guard let result = value.bool else {
            throw AppControlError.invalidArgument("parameters.\(name)")
        }
        return result
    }
}
