import Foundation

enum CampaignBuilder {
    static func data(for raw: CampaignConfiguration) throws -> Data {
        let nodes = max(2, raw.nodes)
        let manualRoots = min(raw.manualRootTimes.count, nodes - 1)
        let sybils = min(
            raw.sybilEvents.reduce(0) { $0 + min(max(1, $1.count), 100_000) },
            nodes - manualRoots - 1
        )
        let arrivals = min(max(0, raw.arrivals), nodes - manualRoots - sybils - 1)
        let reserved = arrivals + manualRoots + sybils
        let events = interventionEvents(
            raw, nodes: nodes, reserved: reserved, manualRootCount: manualRoots,
            sybilLimit: sybils
        )
        let campaign: [String: Any] = [
            "apiVersion": "experiments.fips.network/v1alpha1",
            "kind": "Campaign",
            "metadata": [
                "name": "interactive-root-wave",
                "description": "Interactive Wind Tunnel experiment"
            ],
            "seed": 424_242,
            "engine": [
                "modes": "compact-discrete-event",
                "deterministic": true,
                "variant": "fips-80c956a-baseline"
            ],
            "scale": ["nodes": nodes],
            "topology": topologyConfiguration(raw, nodes: nodes),
            "identities": [
                "initial": ["distribution": "uniform-128"],
                "arrivals": [
                    "count": arrivals,
                    "schedule": [
                        "start": duration(seconds: 1),
                        "interval": duration(seconds: max(0, raw.intervalSeconds))
                    ],
                    "address_policy": "strictly-lower-than-current-root",
                    "attachment": raw.attachment,
                    "attacker_budget": [
                        "mode": "bounded",
                        "operations": reserved,
                        "identities": reserved
                    ]
                ]
            ],
            "transports": transportConfiguration(raw),
            "links": [
                "latency": duration(milliseconds: raw.latencyMilliseconds),
                "jitter": duration(milliseconds: raw.jitterMilliseconds),
                "bandwidth_bps": max(1, raw.bandwidthMbps) * 1_000_000,
                "loss_ppm": min(max(0, raw.lossPPM), 1_000_000),
                "duplication_ppm": 0,
                "ordering": "datagram",
                "mtu_bytes": max(68, raw.mtuBytes),
                "queue_bytes": 1_048_576,
                "drop_policy": "tail-drop"
            ],
            "resources": [
                "assignment": raw.heterogeneousResources ? "heterogeneous" : "uniform",
                "node_profiles": [[
                    "name": "baseline",
                    "cpu_units": max(0, raw.cpuUnitsPerMS),
                    "memory_bytes": max(1, raw.memoryMB) * 1_048_576,
                    "queue_bytes": max(1, raw.nodeQueueKB) * 1_024,
                    "table_entries": max(1, raw.tableEntries)
                ]]
            ],
            "events": events,
            "protocol": [
                "variant": "fips-80c956a-baseline",
                "parameters": [
                    "tree_announce_debounce": duration(milliseconds: raw.debounceMilliseconds),
                    "bloom_update_debounce": duration(milliseconds: raw.debounceMilliseconds),
                    "bloom_max_fpr_ppm": 200_000,
                    "lookup_ttl": min(max(1, raw.lookupTTL), 255),
                    "lookup_attempts": max(1, raw.lookupAttempts),
                    "lookup_backoff": duration(milliseconds: 100),
                    "lookup_jitter": duration(milliseconds: 10),
                    "coord_cache_entries": max(1, raw.coordinateCacheEntries),
                    "coord_cache_ttl": duration(seconds: 5),
                    "parent_hysteresis_ppm":
                        min(max(0, raw.parentHysteresisPercent), 100) * 10_000,
                    "parent_hold_down":
                        duration(milliseconds: Double(max(0, raw.parentHoldDownMilliseconds)))
                ]
            ],
            "traffic": trafficConfiguration(raw),
            "adversaries": adversaryConfiguration(raw),
            "fidelity": [
                "protocol": "semantic-exact",
                "serialization": "executable-codec",
                "bloom": "exact-bits",
                "crypto": "operation-count",
                "billion_node_representation": "not-requested"
            ],
            "accounting": [
                "causal_lineage": true,
                "transport_overhead": true,
                "network_overhead": "configured",
                "reconcile_serialized_frames": true
            ],
            "instrumentation": [
                "root_agreement_by_depth": true,
                "transition_stages": true,
                "causal_cost_ledger": true,
                "queue_wait": true,
                "control_and_useful_bytes": true,
                "quiescence_markers": quiescenceMarkers(raw)
            ],
            "assertions": [
                ["eventually": [
                    "condition": "all_connected_nodes_agree_on_minimum_root",
                    "after_input_quiescence": duration(seconds: 30)
                ]],
                ["always": ["condition": "no_forwarding_loops"]],
                ["always": ["condition": "per_peer_announcements_obey_debounce"]],
                ["reconcile": ["condition": "measured_bytes_equal_serialized_frames"]],
                ["deterministic": [
                    "condition": "same_seed_same_event_order_and_result"
                ]]
            ],
            "objectives": ["maximize": ["control_bytes_per_root_arrival"]]
        ]
        return try JSONSerialization.data(withJSONObject: campaign, options: [.prettyPrinted, .sortedKeys])
    }

    static func duration(seconds: Double) -> String {
        "\(UInt64(max(0, seconds) * 1_000_000_000))ns"
    }

    static func duration(milliseconds: Double) -> String {
        "\(UInt64(max(0, milliseconds) * 1_000_000))ns"
    }

    private static func transportConfiguration(_ raw: CampaignConfiguration) -> [String: Any] {
        guard raw.mixedTransports else { return ["assignment": "all-udp"] }
        return [
            "assignment": "random-mixed",
            "profiles": [
                profile(
                    name: "wifi", type: "wifi", mtu: 1_500, latencyMS: 8,
                    mbps: raw.wifiMbps, lossPPM: 1_000, weight: raw.wifiWeight
                ),
                profile(
                    name: "bluetooth", type: "ble", mtu: 244, latencyMS: 20,
                    mbps: raw.bluetoothMbps, lossPPM: 5_000, weight: raw.bluetoothWeight
                ),
                profile(
                    name: "tor", type: "tor", mtu: 1_280, latencyMS: 200,
                    mbps: raw.torMbps, lossPPM: 500, weight: raw.torWeight
                ),
                profile(
                    name: "ethernet", type: "ethernet", mtu: 1_500, latencyMS: 1,
                    mbps: raw.ethernetMbps, lossPPM: 0, weight: raw.ethernetWeight
                )
            ]
        ]
    }

    static func topologyConfiguration(
        _ raw: CampaignConfiguration, nodes: Int
    ) -> [String: Any] {
        var topology: [String: Any] = [
            "generator": raw.topology,
            "average_degree": max(1, raw.averageDegree),
            "connected": true
        ]
        if raw.topology == "explicit" {
            topology["explicit_edges"] = raw.explicitEdges.map { [$0.from, $0.to] }
        }
        if raw.mediaZonesEnabled {
            let count = min(max(1, raw.mediaZoneCount), nodes)
            topology["media_zones"] = (0..<count).map { zone in
                [
                    "id": "zone-\(zone)",
                    "nodes": Array(stride(from: zone, to: nodes, by: count)),
                    "bandwidth_bps": max(1, raw.mediaZoneBandwidthMbps) * 1_000_000,
                    "latency": duration(milliseconds: raw.mediaZoneLatencyMilliseconds),
                    "loss_ppm": min(max(0, raw.mediaZoneLossPPM), 1_000_000),
                    "mtu_bytes": max(68, raw.mediaZoneMTUBytes),
                    "queue_bytes": max(1, raw.mediaZoneQueueKB) * 1_024
                ] as [String: Any]
            }
        }
        return topology
    }

    private static func trafficConfiguration(_ raw: CampaignConfiguration) -> [String: Any] {
        guard raw.trafficEnabled else { return ["model": "idle"] }
        return [
            "model": raw.trafficModel,
            "rate_bps": max(1, raw.trafficRateMbps) * 1_000_000,
            "payload_bytes": max(1, raw.trafficPayloadBytes),
            "parameters": [
                "flow_count": min(max(1, raw.trafficFlows), 100_000),
                "start_ns": 500_000_000,
                "segments_per_stream": min(max(1, raw.trafficSegmentsPerStream), 10_000),
                "burst_size": min(max(1, raw.trafficBurstSize), 100_000),
                "burst_interval_ns":
                    min(max(1, raw.trafficBurstIntervalMilliseconds), 3_600_000) * 1_000_000
            ]
        ]
    }

    private static func quiescenceMarkers(_ raw: CampaignConfiguration) -> [String] {
        var markers = ["root", "tree"]
        if raw.bloomEnabled { markers.append("bloom") }
        if raw.lookupRecoveryEnabled { markers.append("lookup") }
        return markers
    }

    private static func profile(
        name: String,
        type: String,
        mtu: Int,
        latencyMS: Double,
        mbps: Int,
        lossPPM: Int,
        weight: Int
    ) -> [String: Any] {
        [
            "name": name,
            "type": type,
            "mtu_bytes": mtu,
            "latency": duration(milliseconds: latencyMS),
            "jitter": duration(milliseconds: latencyMS / 4),
            "bandwidth_bps": max(1, mbps) * 1_000_000,
            "loss_ppm": min(max(0, lossPPM), 1_000_000),
            "queue_bytes": 1_048_576,
            "weight": max(0, weight)
        ]
    }
}
