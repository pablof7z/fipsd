import Foundation

enum ClaudeACPProtocol {
    static let adapterPackage = "@agentclientprotocol/claude-agent-acp@0.61.0"

    static func initializeParams() -> JSONValue {
        .object([
            "protocolVersion": .integer(1),
            "clientCapabilities": .object([
                "fs": .object([
                    "readTextFile": .bool(false),
                    "writeTextFile": .bool(false)
                ]),
                "terminal": .bool(false)
            ]),
            "clientInfo": .object([
                "name": .string("FIPS Wind Tunnel"),
                "version": .string("0.2.0")
            ])
        ])
    }

    static func newSessionParams(
        cwd: URL,
        mcpExecutable: URL,
        skill: String
    ) -> JSONValue {
        .object([
            "cwd": .string(cwd.path),
            "mcpServers": .array([
                .object([
                    "name": .string("fips-wind-tunnel"),
                    "command": .string(mcpExecutable.path),
                    "args": .array([]),
                    "env": .array([])
                ])
            ]),
            "_meta": .object([
                "systemPrompt": .object([
                    "type": .string("preset"),
                    "preset": .string("claude_code"),
                    "append": .string(systemPrompt(skill: skill))
                ]),
                "claudeCode": .object([
                    "options": .object([
                        "disallowedTools": .array(
                            nestedAuthoringTools.map(JSONValue.string)
                        )
                    ])
                ])
            ])
        ])
    }

    static func promptParams(sessionID: String, text: String) -> JSONValue {
        .object([
            "sessionId": .string(sessionID),
            "prompt": .array([
                .object([
                    "type": .string("text"),
                    "text": .string(text)
                ])
            ])
        ])
    }

    static func cancelParams(sessionID: String) -> JSONValue {
        .object(["sessionId": .string(sessionID)])
    }

    static func bypassPermissionParams(sessionID: String) -> JSONValue {
        .object([
            "sessionId": .string(sessionID),
            "modeId": .string("bypassPermissions")
        ])
    }

    static func event(from params: JSONValue) -> ClaudeACPEvent? {
        guard let update = params.object?["update"]?.object,
              let kind = update["sessionUpdate"]?.string else { return nil }
        switch kind {
        case "agent_message_chunk":
            guard let text = update["content"]?.object?["text"]?.string else { return nil }
            return .message(id: update["messageId"]?.string, text: text)
        case "tool_call", "tool_call_update":
            guard let id = update["toolCallId"]?.string else { return nil }
            return .activity(
                id: id,
                title: update["title"]?.string ?? "Using a tool",
                status: update["status"]?.string
            )
        case "usage_update":
            return .usage(
                used: update["used"]?.int ?? 0,
                size: update["size"]?.int ?? 0
            )
        case "plan":
            return .activity(id: "plan", title: "Planning", status: nil)
        default:
            return nil
        }
    }

    static func permission(
        rpcID: JSONValue,
        params: JSONValue
    ) -> ClaudePermissionRequest? {
        guard let object = params.object,
              let tool = object["toolCall"]?.object,
              let toolID = tool["toolCallId"]?.string else { return nil }
        let options: [ClaudePermissionOption] = object["options"]?.array?.compactMap { value in
            guard let item = value.object,
                  let id = item["optionId"]?.string,
                  let name = item["name"]?.string else { return nil }
            return ClaudePermissionOption(
                id: id,
                name: name,
                kind: item["kind"]?.string ?? ""
            )
        } ?? []
        return ClaudePermissionRequest(
            rpcID: rpcID,
            toolCallID: toolID,
            title: tool["title"]?.string ?? "Claude wants to use a tool",
            options: options
        )
    }

    static func permissionResult(optionID: String?) -> JSONValue {
        let outcome: JSONValue = if let optionID {
            .object([
                "outcome": .string("selected"),
                "optionId": .string(optionID)
            ])
        } else {
            .object(["outcome": .string("cancelled")])
        }
        return .object(["outcome": outcome])
    }

    private static let nestedAuthoringTools = [
        "mcp__fips-wind-tunnel__wind_tunnel_start_experiment",
        "mcp__fips-wind-tunnel__wind_tunnel_amend_experiment",
        "mcp__fips-wind-tunnel__wind_tunnel_get_skill"
    ]

    private static func systemPrompt(skill: String) -> String { """
    You are the embedded experiment agent in FIPS Wind Tunnel. The \
    fips-wind-tunnel MCP server controls and inspects the visible app. Use its \
    tools whenever the user asks about current state, starts or amends an \
    experiment, controls playback, or requests analysis. You are the only \
    reasoning layer: never ask another model to reinterpret instructions. Apply \
    simple configuration through wind_tunnel_set_parameters, complete declarative \
    scenarios through wind_tunnel_run_campaign, and forward-only changes through \
    wind_tunnel_inject_event. Save reusable Campaigns with \
    wind_tunnel_save_experiment; use wind_tunnel_list_experiments and \
    wind_tunnel_rerun_experiment when the user asks to find or rerun a saved \
    experiment. Then wait for completion and inspect state or analysis. Ground \
    claims in MCP state and artifacts. Preserve fidelity and provenance labels; \
    do not imply wire-exactness, agreement, scale, or success beyond the returned \
    evidence.

    The complete Wind Tunnel operating skill is included below and is authoritative \
    for this session. Do not call wind_tunnel_get_skill: it would only reload text \
    you already have. Do not search or read knowledge resources for routine direct \
    control, playback, save, list, or rerun requests. Use targeted knowledge only \
    when protocol facts, raw Campaign schema details, or evidence interpretation \
    actually require sources.

    <wind-tunnel-operating-skill>
    \(skill)
    </wind-tunnel-operating-skill>
    """
    }
}
