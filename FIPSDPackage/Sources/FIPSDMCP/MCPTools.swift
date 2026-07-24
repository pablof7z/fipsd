import Foundation

enum MCPTools {
    static var definitions: [[String: Any]] { [
        tool(
            "wind_tunnel_launch",
            "Launch FIPS Wind Tunnel or return its current state.",
            schema(),
            readOnly: false
        ),
        tool(
            "wind_tunnel_get_skill",
            "Return the full operating skill for external clients that did not receive it at session startup.",
            schema(),
            readOnly: true
        ),
        tool(
            "wind_tunnel_list_knowledge",
            "Search source-backed FIPS protocol, product, schema, example, and skill resources.",
            schema(properties: [
                "query": string("Optional case-insensitive title, path, or description search."),
                "collection": enumeration([
                    "skill", "product", "schema", "example", "protocol"
                ]),
                "limit": integer("Maximum results from 1 through 500.")
            ]),
            readOnly: true
        ),
        tool(
            "wind_tunnel_read_knowledge",
            "Read one exact URI returned by the FIPS knowledge catalog.",
            schema(
                properties: [
                    "uri": string("Exact fips-wind-tunnel knowledge resource URI.")
                ],
                required: ["uri"]
            ),
            readOnly: true
        ),
        tool(
            "wind_tunnel_get_state",
            "Read the rendered experiment, topology sample, traffic, cursor, run, and configuration.",
            schema(properties: [
                "limit": integer("Maximum nodes and edges to return, from 0 to 1000.")
            ]),
            readOnly: true
        ),
        tool(
            "wind_tunnel_start_experiment",
            "Author and start a new experiment from natural language in the visible app.",
            schema(
                properties: [
                    "prompt": string("Complete experiment request."),
                    "model": model()
                ],
                required: ["prompt"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_amend_experiment",
            "Change the current run from its rendered cursor using natural language and exact state.",
            schema(
                properties: [
                    "prompt": string("Forward-looking change to the current experiment."),
                    "model": model()
                ],
                required: ["prompt"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_playback",
            "Play, pause, stop, seek, step, or change playback speed.",
            schema(
                properties: [
                    "action": enumeration([
                        "play", "pause", "toggle", "stop", "step_forward",
                        "step_backward", "seek", "set_speed"
                    ]),
                    "time_ns": integer("Seek destination in virtual nanoseconds."),
                    "time_seconds": number("Seek destination in virtual seconds."),
                    "speed": number("Playback multiplier from 0.01 through 1000.")
                ],
                required: ["action"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_set_parameters",
            "Update direct-control parameters, optionally starting a new deterministic run.",
            schema(
                properties: [
                    "parameters": [
                        "type": "object",
                        "description": parameterDescription,
                        "additionalProperties": true
                    ],
                    "run": [
                        "type": "boolean",
                        "description": "Immediately start the updated configured scenario."
                    ]
                ],
                required: ["parameters"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_run_campaign",
            "Run a complete Campaign v1alpha1 JSON object in the visible app.",
            schema(
                properties: [
                    "campaign": [
                        "type": "object",
                        "description": "Complete schema-valid Campaign document."
                    ],
                    "description": string("Optional provenance description.")
                ],
                required: ["campaign"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_inject_event",
            "Inject a supported event after the cursor and replay immutable history into a new branch.",
            schema(
                properties: [
                    "action": string("Campaign amendment action."),
                    "target": [
                        "description": "Numeric node/edge target or named transport target.",
                        "anyOf": [["type": "integer"], ["type": "string"]]
                    ],
                    "parameters": [
                        "type": "object",
                        "description": "Action-specific event parameters."
                    ],
                    "after_ms": number("Delay from the current cursor; defaults to 100 ms."),
                    "at_ns": integer("Absolute virtual event time."),
                    "id": string("Optional deterministic event ID.")
                ],
                required: ["action"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_save_experiment",
            "Save the exact active Campaign to the durable local experiment library.",
            schema(properties: [
                "name": string(
                    "Optional display name; defaults to Campaign metadata.name."
                ),
                "description": string(
                    "Optional description; defaults to Campaign metadata.description."
                )
            ]),
            readOnly: false
        ),
        tool(
            "wind_tunnel_list_experiments",
            "List durable saved experiments with identifiers, checksums, and provenance.",
            schema(),
            readOnly: true
        ),
        tool(
            "wind_tunnel_rerun_experiment",
            "Rerun the exact Campaign bytes from a durable saved experiment.",
            schema(
                properties: [
                    "id": string(
                        "Saved experiment identifier from wind_tunnel_list_experiments."
                    )
                ],
                required: ["id"]
            ),
            readOnly: false
        ),
        tool(
            "wind_tunnel_get_analysis",
            "Read fidelity, causal stages, bottlenecks, root impacts, and evidence location.",
            schema(),
            readOnly: true
        ),
        tool(
            "wind_tunnel_explain",
            "Return a concise evidence-grounded explanation plus current state and analysis.",
            schema(properties: [
                "focus": string("Optional question or subsystem to emphasize.")
            ]),
            readOnly: true
        ),
        tool(
            "wind_tunnel_wait_until_idle",
            "Wait until authoring and engine execution finish, then return current state.",
            schema(properties: [
                "timeout_ms": integer("Maximum wait, capped at 300000 ms.")
            ]),
            readOnly: true
        )
    ] }

    private static let parameterDescription = """
    Supported keys: nodes, arrivals, interval_seconds, topology, average_degree,
    attachment, bandwidth_mbps, latency_ms, jitter_ms, loss_ppm, mtu_bytes,
    debounce_ms, bloom_enabled, lookup_recovery_enabled, mixed_transports,
    wifi_weight, wifi_mbps, bluetooth_weight, bluetooth_mbps, tor_weight,
    tor_mbps, ethernet_weight, ethernet_mbps, traffic_enabled, traffic_model,
    traffic_flows, traffic_payload_bytes, traffic_rate_mbps,
    traffic_segments_per_stream, traffic_burst_size, traffic_burst_interval_ms,
    heterogeneous_resources, cpu_units_per_ms, memory_mb, node_queue_kb,
    table_entries, coordinate_cache_entries, lookup_ttl, lookup_attempts, and
    lookup_storm_count.
    """

    private static func tool(
        _ name: String,
        _ description: String,
        _ inputSchema: [String: Any],
        readOnly: Bool
    ) -> [String: Any] {
        [
            "name": name,
            "title": name.replacingOccurrences(of: "_", with: " ").capitalized,
            "description": description,
            "inputSchema": inputSchema,
            "annotations": [
                "readOnlyHint": readOnly,
                "destructiveHint": name == "wind_tunnel_playback",
                "idempotentHint": readOnly,
                "openWorldHint": false
            ]
        ]
    }

    private static func schema(
        properties: [String: Any] = [:],
        required: [String] = []
    ) -> [String: Any] {
        var result: [String: Any] = [
            "type": "object",
            "properties": properties,
            "additionalProperties": false
        ]
        if !required.isEmpty { result["required"] = required }
        return result
    }

    private static func string(_ description: String) -> [String: Any] {
        ["type": "string", "description": description]
    }

    private static func integer(_ description: String) -> [String: Any] {
        ["type": "integer", "description": description]
    }

    private static func number(_ description: String) -> [String: Any] {
        ["type": "number", "description": description]
    }

    private static func enumeration(_ values: [String]) -> [String: Any] {
        ["type": "string", "enum": values]
    }

    private static func model() -> [String: Any] {
        [
            "type": "string",
            "enum": ["auto", "sonnet", "haiku", "opus", "codex"],
            "description": "Local authoring model; defaults to the app selection."
        ]
    }
}
