import Foundation

enum PromptAuthorError: LocalizedError {
    case providerUnavailable(String)
    case providerFailed(String)
    case emptyResult

    var errorDescription: String? {
        switch self {
        case let .providerUnavailable(name): "Local provider \(name) was not found."
        case let .providerFailed(message): "Local authoring provider failed: \(message)"
        case .emptyResult: "The local authoring provider returned no campaign."
        }
    }
}

struct PromptAuthor: Sendable {
    func generate(
        prompt: String,
        provider requested: AuthoringProvider,
        template: Data
    ) async throws -> (Data, AuthoringProvider) {
        if let intent = ExplicitTransferIntent.parse(prompt) {
            return (try intent.applying(to: template), .automatic)
        }
        let provider = try resolve(requested)
        let templateText = String(decoding: template, as: UTF8.self)
        let system = Self.systemPrompt + "\n\nValid baseline campaign:\n" + templateText
        let result: String
        switch provider {
        case .claudeSonnet, .claudeHaiku, .claudeOpus:
            result = try await runClaude(
                prompt: prompt,
                system: system,
                model: provider.claudeModel!
            )
        case .codex:
            result = try await runCodex(prompt: prompt, system: system)
        case .automatic:
            fatalError("automatic is resolved before invocation")
        }
        guard let data = extractJSONObject(from: result) else {
            throw PromptAuthorError.emptyResult
        }
        let canonical = try canonicalizeSupportedAliases(data)
        let authored = try ExplicitTransferIntent.parse(prompt)?.applying(to: canonical)
            ?? canonical
        return (authored, provider)
    }

    func amend(
        prompt: String,
        provider requested: AuthoringProvider,
        campaign: Data,
        renderedState: LiveRunContext,
        at timeNS: UInt64
    ) async throws -> (Data, AuthoringProvider) {
        let bridgeIntent = BridgeReplacementIntent.parse(
            prompt,
            renderedState: renderedState.data
        )
        let current = String(decoding: campaign, as: UTF8.self)
        let snapshot = String(decoding: renderedState.data, as: UTF8.self)
        let system = Self.interventionPrompt
            + "\n\nCurrent campaign to amend:\n" + current
            + "\n\nExact rendered state at the user's cursor:\n" + snapshot
            + "\n\nSchedule new events no earlier than \(timeNS)ns."
        do {
            let provider = try resolve(requested)
            let result = switch provider {
            case .claudeSonnet, .claudeHaiku, .claudeOpus:
                try await runClaude(
                    prompt: prompt,
                    system: system,
                    model: provider.claudeModel!
                )
            case .codex: try await runCodex(prompt: prompt, system: system)
            case .automatic: fatalError("automatic is resolved before invocation")
            }
            guard let data = extractJSONObject(from: result) else {
                throw PromptAuthorError.emptyResult
            }
            if let bridgeIntent {
                return (
                    try bridgeIntent.applying(
                        to: campaign,
                        at: timeNS,
                        realizedArrivals: renderedState.realizedArrivals
                    ),
                    provider
                )
            }
            let amended = try CampaignAmendment.applying(
                data,
                to: campaign,
                noEarlierThan: timeNS,
                realizedArrivals: renderedState.realizedArrivals,
                attachmentNode: renderedState.currentRootID
            )
            return (amended, provider)
        } catch {
            if let bridgeIntent {
                return (
                    try bridgeIntent.applying(
                        to: campaign,
                        at: timeNS,
                        realizedArrivals: renderedState.realizedArrivals
                    ),
                    .automatic
                )
            }
            guard let fallback = LiveInterventionIntent.parse(prompt) else { throw error }
            return (
                try fallback.applying(
                    to: campaign,
                    at: timeNS,
                    attachmentNode: renderedState.currentRootID
                ),
                .automatic
            )
        }
    }

    private func resolve(_ provider: AuthoringProvider) throws -> AuthoringProvider {
        if provider != .automatic {
            guard let name = provider.executableName,
                  executable(named: name) != nil else {
                throw PromptAuthorError.providerUnavailable(provider.rawValue)
            }
            return provider
        }
        if executable(named: "claude") != nil { return .claudeSonnet }
        if executable(named: "codex") != nil { return .codex }
        throw PromptAuthorError.providerUnavailable("Claude or Codex")
    }

    private func runClaude(
        prompt: String,
        system: String,
        model: String
    ) async throws -> String {
        guard let executable = executable(named: "claude") else {
            throw PromptAuthorError.providerUnavailable("Claude")
        }
        let output = try await run(
            executable: executable,
            arguments: [
                "-p", "--model", model, "--tools", "", "--system-prompt", system,
                "--output-format", "json", "--no-session-persistence"
            ],
            input: prompt
        )
        if let data = output.data(using: .utf8),
           let wrapper = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let result = wrapper["result"] as? String { return result }
        return output
    }

    private func runCodex(prompt: String, system: String) async throws -> String {
        guard let executable = executable(named: "codex") else {
            throw PromptAuthorError.providerUnavailable("Codex")
        }
        let outputURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("fipsd-codex-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: outputURL) }
        _ = try await run(
            executable: executable,
            arguments: [
                "exec", "--ephemeral", "--sandbox", "read-only",
                "--ignore-user-config", "-o", outputURL.path, "-"
            ],
            input: system + "\n\nUser experiment:\n" + prompt
        )
        return try String(contentsOf: outputURL, encoding: .utf8)
    }

    private func run(executable: URL, arguments: [String], input: String) async throws -> String {
        let process = Process()
        let stdin = Pipe()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = executable
        process.arguments = arguments
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        stdin.fileHandleForWriting.write(Data(input.utf8))
        try stdin.fileHandleForWriting.close()
        let output = try stdout.fileHandleForReading.readToEnd() ?? Data()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            let error = try stderr.fileHandleForReading.readToEnd() ?? Data()
            throw PromptAuthorError.providerFailed(String(decoding: error, as: UTF8.self))
        }
        return String(decoding: output, as: UTF8.self)
    }

    private func executable(named name: String) -> URL? {
        let candidates = [
            "/Users/\(NSUserName())/.local/bin/\(name)",
            "/opt/homebrew/bin/\(name)",
            "/usr/local/bin/\(name)"
        ]
        return candidates.first(where: FileManager.default.isExecutableFile(atPath:)).map(URL.init(fileURLWithPath:))
    }

    private func extractJSONObject(from text: String) -> Data? {
        guard let start = text.firstIndex(of: "{"),
              let end = text.lastIndex(of: "}") else { return nil }
        let candidate = Data(text[start...end].utf8)
        guard (try? JSONSerialization.jsonObject(with: candidate)) != nil else { return nil }
        return candidate
    }

    private func canonicalizeSupportedAliases(_ data: Data) throws -> Data {
        guard var campaign = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              var identities = campaign["identities"] as? [String: Any],
              var arrivals = identities["arrivals"] as? [String: Any] else { return data }
        if let attachment = arrivals["attachment"] as? String,
           ["deterministic", "seeded", "different-points"].contains(attachment) {
            arrivals["attachment"] = "random"
            identities["arrivals"] = arrivals
            campaign["identities"] = identities
        }
        return try JSONSerialization.data(withJSONObject: campaign, options: [.prettyPrinted, .sortedKeys])
    }

    private static let systemPrompt = """
    You author deterministic FIPS Wind Tunnel Campaign v1alpha1 documents.
    Return one JSON object only, with no markdown or commentary. Preserve every required field
    from the baseline. Translate the request into declarative topology, identity arrivals,
    schedule, links, protocol parameters, traffic, assertions, and fidelity. Use only supported
    baseline values unless the request explicitly changes them. Topology must be explicit, chain,
    balanced-tree, random-regular, or scale-free. Arrival attachment must be current-root, leaf,
    hub, articulation, or random; use random for different seeded attachment points. Transport
    assignment must be all-udp, all-tcp, all-ethernet, or random-mixed. For random-mixed,
    preserve or author profiles with type wifi, ble, tor, ethernet, udp, tcp, or nym plus
    bandwidth_bps, latency, mtu_bytes, loss_ppm, queue_bytes, and weight. For visible payload
    movement, use a supported non-idle traffic model, a flow_count no greater than 100000,
    payload_bytes, and rate_bps. Supported models include persistent-streams with
    segments_per_stream and bursty with burst_size plus burst_interval_ns. A request for one
    concrete download or object transfer must use model explicit-transfers and a transfers array.
    Each transfer has id, numeric source and destination node IDs, total_bytes,
    visualization_chunk_bytes, and start. Use an explicit chain when the requested endpoints must
    not have a direct link; the engine computes the route through intermediate nodes. When lookup recovery
    is requested, include lookup in
    quiescence_markers and keep a non-idle traffic model; synchronized-session-rekey is then a
    supported timed event. expire-coordinate-cache followed by simultaneous-lookups with a bounded
    parameters.count creates a deterministic lookup storm. Include bloom separately when requested.
    fail-transport-class and restore-transport-class target one authored profile name.
    swap-parent-ancestry drives one same-root MMP-cost parent switch. alternate-parent-quality
    accepts parameters.cycles and parameters.interval to test hysteresis and hold-down.
    attach-authenticated-sybils requires adversaries.mode authenticated-protocol-valid, an at
    time, bounded parameters.count, attachment, interval, and an identity/operation budget.
    Keep quiescence_markers to root and tree otherwise. Never emit commands, scripts,
    paths, tools, or prose. The result is untrusted and will be schema-validated before execution.
    """

    private static let interventionPrompt = """
    Author a forward-only amendment to a running deterministic FIPS Wind Tunnel campaign.
    Return only one JSON object with events, cancel_future_event_ids, and
    stop_scheduled_arrivals. Do not return the campaign. The current campaign and the exact
    rendered state are supplied below. Nodes include stable numeric IDs, human labels, active
    state, and joined_at_ns; edges include the exact numeric IDs required by link events.
    Express changes as new time-stamped events. Supported live changes include
    set-link-conditions, restore-link-conditions, introduce-lower-root-node, introduce-node,
    disappear-node, reappear-node, partition-network, merge-network, fail-transport-class,
    restore-transport-class, synchronized-session-rekey, expire-coordinate-cache, and
    simultaneous-lookups. A link event targets its numeric edge ID and can set bandwidth_bps,
    latency, jitter, loss_ppm, and mtu_bytes. A node lifecycle event targets its numeric node ID.
    Use introduce-node with parameters.attachments containing every numeric neighbor when a
    normal node joins; use introduce-lower-root-node only when it must become root.
    For repeated changes, emit a finite sequence at explicit virtual times. When asked to stop
    recurring joins, set stop_scheduled_arrivals true. To remove the oldest nodes until a target
    active count remains, use nodes_oldest_first and emit disappear-node events at the requested
    interval, excluding enough newest nodes to reach the target. cancel_future_event_ids may
    contain only IDs already listed in scheduled_campaign_events. Never schedule before the
    supplied minimum time. Never emit commands, paths, tools, markdown, or prose.
    """
}
