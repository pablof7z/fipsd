import Foundation
import Testing
@testable import FIPSDFeature

@Test func claudeSessionReceivesWindTunnelMCP() throws {
    let params = ClaudeACPProtocol.newSessionParams(
        cwd: URL(fileURLWithPath: "/tmp/work", isDirectory: true),
        mcpExecutable: URL(fileURLWithPath: "/tmp/fips-wind-tunnel-mcp"),
        skill: "# Full operating skill\nUse direct tools."
    )
    let object = try #require(params.object)
    let servers = try #require(object["mcpServers"]?.array)
    let server = try #require(servers.first?.object)
    #expect(server["name"]?.string == "fips-wind-tunnel")
    #expect(server["command"]?.string == "/tmp/fips-wind-tunnel-mcp")
    #expect(server["args"]?.array == [])
    #expect(server["env"]?.array == [])

    let metadata = try #require(object["_meta"]?.object)
    let prompt = try #require(metadata["systemPrompt"]?.object)
    #expect(prompt["preset"]?.string == "claude_code")
    #expect(prompt["append"]?.string?.contains("fips-wind-tunnel MCP") == true)
    #expect(prompt["append"]?.string?.contains("# Full operating skill") == true)
    #expect(prompt["append"]?.string?.contains("Do not call wind_tunnel_get_skill") == true)
    let claudeCode = try #require(metadata["claudeCode"]?.object)
    let options = try #require(claudeCode["options"]?.object)
    let disabled = try #require(options["disallowedTools"]?.array)
    #expect(
        disabled.compactMap(\.string) == [
            "mcp__fips-wind-tunnel__wind_tunnel_start_experiment",
            "mcp__fips-wind-tunnel__wind_tunnel_amend_experiment",
            "mcp__fips-wind-tunnel__wind_tunnel_get_skill"
        ]
    )
    #expect(
        ClaudeACPProtocol.bypassPermissionParams(sessionID: "session-1")
            == .object([
                "sessionId": .string("session-1"),
                "modeId": .string("bypassPermissions")
            ])
    )
}

@Test func claudeMessageUpdateDecodesACPChunk() {
    let event = ClaudeACPProtocol.event(
        from: .object([
            "sessionId": .string("session-1"),
            "update": .object([
                "sessionUpdate": .string("agent_message_chunk"),
                "messageId": .string("message-1"),
                "content": .object([
                    "type": .string("text"),
                    "text": .string("Current root is node 3.")
                ])
            ])
        ])
    )
    #expect(
        event == .message(
            id: "message-1",
            text: "Current root is node 3."
        )
    )
}

@Test func claudePermissionRoundTripsSelectedOption() throws {
    let request = ClaudeACPProtocol.permission(
        rpcID: .integer(41),
        params: .object([
            "toolCall": .object([
                "toolCallId": .string("tool-7"),
                "title": .string("Inspect workbench state")
            ]),
            "options": .array([
                .object([
                    "optionId": .string("allow_once"),
                    "name": .string("Allow"),
                    "kind": .string("allow_once")
                ]),
                .object([
                    "optionId": .string("reject_once"),
                    "name": .string("Deny"),
                    "kind": .string("reject_once")
                ])
            ])
        ])
    )
    let decoded = try #require(request)
    #expect(decoded.title == "Inspect workbench state")
    #expect(decoded.options.map(\.id) == ["allow_once", "reject_once"])
    #expect(
        ClaudeACPProtocol.permissionResult(optionID: "allow_once")
            == .object([
                "outcome": .object([
                    "outcome": .string("selected"),
                    "optionId": .string("allow_once")
                ])
            ])
    )
}

@Test func claudeExecutableOverridesAreResolved() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(
        at: directory,
        withIntermediateDirectories: true
    )
    defer { try? FileManager.default.removeItem(at: directory) }
    let npx = directory.appendingPathComponent("npx")
    let mcp = directory.appendingPathComponent("fips-wind-tunnel-mcp")
    try Data("#!/bin/sh\n".utf8).write(to: npx)
    try Data("#!/bin/sh\n".utf8).write(to: mcp)
    try FileManager.default.setAttributes(
        [.posixPermissions: 0o755],
        ofItemAtPath: npx.path
    )
    try FileManager.default.setAttributes(
        [.posixPermissions: 0o755],
        ofItemAtPath: mcp.path
    )

    #expect(
        try ClaudeACPClient.npxExecutable(
            environment: ["FIPS_WIND_TUNNEL_NPX": npx.path]
        ) == npx
    )
    #expect(
        try ClaudeACPClient.mcpExecutable(
            environment: ["FIPS_WIND_TUNNEL_MCP": mcp.path]
        ) == mcp
    )
}

@Test func claudeSkillLoadsFromConfiguredWorkspace() throws {
    let root = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let skillDirectory = root.appendingPathComponent(
        "skills/fips-wind-tunnel",
        isDirectory: true
    )
    try FileManager.default.createDirectory(
        at: skillDirectory,
        withIntermediateDirectories: true
    )
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("# Test Wind Tunnel skill\nOperate directly.".utf8).write(
        to: skillDirectory.appendingPathComponent("SKILL.md"),
        options: .atomic
    )

    let loaded = try ClaudeACPClient.windTunnelSkill(
        environment: ["FIPS_WIND_TUNNEL_WORKSPACE": root.path]
    )

    #expect(loaded.contains("# Test Wind Tunnel skill"))
    #expect(loaded.contains("Operate directly."))
}
