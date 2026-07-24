import Foundation

final class MCPServer {
    private let client = AppControlClient()
    let knowledge = MCPKnowledge()
    private let protocolVersion = "2025-06-18"

    func run() {
        while let line = readLine(strippingNewline: true) {
            guard !line.isEmpty,
                  let data = line.data(using: .utf8),
                  let request = try? JSONSerialization.jsonObject(with: data)
                    as? [String: Any] else {
                write(error(id: NSNull(), code: -32700, message: "Parse error"))
                continue
            }
            if let response = response(for: request) { write(response) }
        }
    }

    func response(for request: [String: Any]) -> [String: Any]? {
        guard request["jsonrpc"] as? String == "2.0",
              let method = request["method"] as? String else {
            return error(
                id: request["id"] ?? NSNull(),
                code: -32600,
                message: "Invalid Request"
            )
        }
        guard let id = request["id"] else { return nil }
        do {
            let result: [String: Any] = switch method {
            case "initialize":
                initializeResult(request["params"])
            case "ping": [:]
            case "tools/list": ["tools": MCPTools.definitions]
            case "tools/call": try callTool(request["params"])
            case "resources/list": ["resources": knowledge.list()]
            case "resources/read": try readResource(request["params"])
            case "prompts/list": ["prompts": promptDefinitions]
            case "prompts/get": try getPrompt(request["params"])
            default: throw MCPServerError.unknownMethod
            }
            return ["jsonrpc": "2.0", "id": id, "result": result]
        } catch MCPServerError.unknownMethod {
            return error(id: id, code: -32601, message: "Method not found")
        } catch {
            return self.error(id: id, code: -32602, message: error.localizedDescription)
        }
    }

    private func initializeResult(_ raw: Any?) -> [String: Any] {
        let params = raw as? [String: Any]
        let requested = params?["protocolVersion"] as? String
        let supported = ["2025-06-18", "2025-03-26", "2024-11-05"]
        return [
            "protocolVersion": supported.contains(requested ?? "")
                ? requested! : protocolVersion,
            "capabilities": [
                "tools": ["listChanged": false],
                "resources": ["subscribe": false, "listChanged": false],
                "prompts": ["listChanged": false]
            ],
            "serverInfo": [
                "name": "fips-wind-tunnel",
                "title": "FIPS Protocol Wind Tunnel",
                "version": "0.1.0"
            ],
            "instructions": """
            Control the visible local FIPS Wind Tunnel app. Inspect state before
            mutating a live run. Use natural-language amendments for semantic
            changes and inject_event for exact supported Campaign events.
            """
        ]
    }

    private func callTool(_ raw: Any?) throws -> [String: Any] {
        guard let params = raw as? [String: Any],
              let name = params["name"] as? String else {
            throw MCPServerError.invalidToolCall
        }
        let arguments = params["arguments"] as? [String: Any] ?? [:]
        let result: Any
        do {
            switch name {
            case "wind_tunnel_launch":
                result = try client.launch()
            case "wind_tunnel_get_state":
                result = try client.call(command: "get_state", arguments: arguments)
            case "wind_tunnel_start_experiment":
                result = try client.call(command: "start_experiment", arguments: arguments)
            case "wind_tunnel_amend_experiment":
                result = try client.call(command: "amend_experiment", arguments: arguments)
            case "wind_tunnel_playback":
                result = try client.call(command: "playback", arguments: arguments)
            case "wind_tunnel_set_parameters":
                result = try client.call(command: "set_parameters", arguments: arguments)
            case "wind_tunnel_run_campaign":
                result = try client.call(command: "run_campaign", arguments: arguments)
            case "wind_tunnel_inject_event":
                result = try client.call(command: "inject_event", arguments: arguments)
            case "wind_tunnel_save_experiment":
                result = try client.call(command: "save_experiment", arguments: arguments)
            case "wind_tunnel_list_experiments":
                result = try client.call(command: "list_experiments")
            case "wind_tunnel_rerun_experiment":
                result = try client.call(command: "rerun_experiment", arguments: arguments)
            case "wind_tunnel_get_analysis":
                result = try client.call(command: "get_analysis")
            case "wind_tunnel_explain":
                result = try client.call(command: "explain", arguments: arguments)
            case "wind_tunnel_wait_until_idle":
                result = try waitUntilIdle(arguments)
            case "wind_tunnel_get_skill":
                result = try knowledge.skill()
            case "wind_tunnel_list_knowledge":
                result = knowledge.search(arguments)
            case "wind_tunnel_read_knowledge":
                guard let uri = arguments["uri"] as? String else {
                    throw MCPServerError.invalidToolCall
                }
                result = try knowledge.read(uri: uri)
            default:
                throw MCPServerError.unknownTool(name)
            }
        } catch {
            return [
                "content": [["type": "text", "text": error.localizedDescription]],
                "isError": true
            ]
        }
        let text = try prettyJSON(result)
        return [
            "content": [["type": "text", "text": text]],
            "structuredContent": result,
            "isError": false
        ]
    }

    private func waitUntilIdle(_ arguments: [String: Any]) throws -> Any {
        let requested = (arguments["timeout_ms"] as? NSNumber)?.doubleValue ?? 60_000
        let timeout = min(max(requested, 0), 300_000) / 1_000
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            let state = try client.call(command: "get_state", arguments: ["limit": 20])
            if let object = state as? [String: Any],
               object["is_running"] as? Bool == false {
                return state
            }
            Thread.sleep(forTimeInterval: 0.2)
        } while Date() < deadline
        throw MCPServerError.waitTimedOut
    }

    func prettyJSON(_ value: Any) throws -> String {
        guard JSONSerialization.isValidJSONObject(value) else {
            return String(describing: value)
        }
        let data = try JSONSerialization.data(
            withJSONObject: value,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
        return String(decoding: data, as: UTF8.self)
    }

    private func error(id: Any, code: Int, message: String) -> [String: Any] {
        [
            "jsonrpc": "2.0",
            "id": id,
            "error": ["code": code, "message": message]
        ]
    }

    private func write(_ object: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: object),
              let line = String(data: data, encoding: .utf8) else { return }
        FileHandle.standardOutput.write(Data((line + "\n").utf8))
    }
}

enum MCPServerError: LocalizedError {
    case invalidToolCall
    case unknownTool(String)
    case unknownMethod
    case waitTimedOut

    var errorDescription: String? {
        switch self {
        case .invalidToolCall: "Invalid tools/call parameters."
        case let .unknownTool(name): "Unknown tool \(name)."
        case .unknownMethod: "Method not found."
        case .waitTimedOut: "The experiment did not become idle before the timeout."
        }
    }
}
