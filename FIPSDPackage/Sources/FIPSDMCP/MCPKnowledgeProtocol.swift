import Foundation

extension MCPServer {
    var promptDefinitions: [[String: Any]] {
        [[
            "name": "fips_wind_tunnel_expert",
            "title": "FIPS Wind Tunnel Expert",
            "description": """
            Supply source-backed operating guidance to an external MCP client
            that was not initialized with the complete embedded skill.
            """,
            "arguments": [[
                "name": "task",
                "description": "The FIPS question or experiment task to perform.",
                "required": false
            ]]
        ]]
    }

    func readResource(_ raw: Any?) throws -> [String: Any] {
        guard let params = raw as? [String: Any],
              let uri = params["uri"] as? String else {
            throw MCPServerError.invalidToolCall
        }
        return ["contents": [try knowledge.read(uri: uri)]]
    }

    func getPrompt(_ raw: Any?) throws -> [String: Any] {
        guard let params = raw as? [String: Any],
              params["name"] as? String == "fips_wind_tunnel_expert" else {
            throw MCPServerError.invalidToolCall
        }
        let arguments = params["arguments"] as? [String: Any] ?? [:]
        let task = arguments["task"] as? String
            ?? "Inspect the current experiment and explain what is happening."
        let skill = try knowledge.skill()
        let text = skill["text"] as? String ?? ""
        return [
            "description": "FIPS Wind Tunnel expert workflow with source routing.",
            "messages": [[
                "role": "user",
                "content": [
                    "type": "text",
                    "text": """
                    Apply the following FIPS Wind Tunnel skill faithfully.

                    \(text)

                    Current task:
                    \(task)
                    """
                ]
            ]]
        ]
    }
}
