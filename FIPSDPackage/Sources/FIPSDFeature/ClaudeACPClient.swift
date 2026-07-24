import Foundation

@MainActor
final class ClaudeACPClient {
    private struct PendingRequest {
        let method: String
        let continuation: CheckedContinuation<JSONValue, Error>
    }

    private var process: Process?
    private var input: FileHandle?
    private var output: FileHandle?
    private var errorOutput: FileHandle?
    private var outputBuffer = Data()
    private var diagnostics = ""
    private var nextID: Int64 = 1
    private var pending: [Int64: PendingRequest] = [:]
    private var sessionID: String?
    private var intentionalShutdown = false
    private var eventHandler: ((ClaudeACPEvent) -> Void)?

    func connect(
        cwd: URL,
        model: String? = nil,
        eventHandler: @escaping (ClaudeACPEvent) -> Void
    ) async throws {
        guard process == nil else { return }
        self.eventHandler = eventHandler
        let npx = try Self.npxExecutable()
        let mcp = try Self.mcpExecutable()
        let skill = try Self.windTunnelSkill()
        try launch(npx: npx, model: model)
        do {
            let initialized = try await request(
                "initialize",
                params: ClaudeACPProtocol.initializeParams()
            )
            guard initialized.object?["protocolVersion"]?.int == 1 else {
                throw ClaudeACPError.invalidResponse("unsupported protocol version")
            }
            let opened = try await request(
                "session/new",
                params: ClaudeACPProtocol.newSessionParams(
                    cwd: cwd,
                    mcpExecutable: mcp,
                    skill: skill
                )
            )
            guard let id = opened.object?["sessionId"]?.string else {
                throw ClaudeACPError.invalidResponse("session/new omitted sessionId")
            }
            let modes = opened.object?["modes"]?.object?["availableModes"]?.array ?? []
            guard modes.contains(where: {
                $0.object?["id"]?.string == "bypassPermissions"
            }) else {
                throw ClaudeACPError.invalidResponse(
                    "Claude did not offer bypassPermissions mode"
                )
            }
            _ = try await request(
                "session/set_mode",
                params: ClaudeACPProtocol.bypassPermissionParams(sessionID: id)
            )
            sessionID = id
        } catch {
            stop()
            throw error
        }
    }

    func prompt(_ text: String) async throws -> String {
        guard let sessionID else { throw ClaudeACPError.notConnected }
        let response = try await request(
            "session/prompt",
            params: ClaudeACPProtocol.promptParams(
                sessionID: sessionID,
                text: text
            ),
            timeout: nil
        )
        guard let reason = response.object?["stopReason"]?.string else {
            throw ClaudeACPError.invalidResponse("session/prompt omitted stopReason")
        }
        return reason
    }

    /// Injects `text` into the turn currently running via the ACP steering
    /// extension (`_session/steering`), rather than queuing it as a separate
    /// `session/prompt`. Its output streams through the existing `session/update`
    /// event handler, not this call's return value.
    func steer(_ text: String) async throws {
        guard let sessionID else { throw ClaudeACPError.notConnected }
        _ = try await request(
            "_session/steering",
            params: ClaudeACPProtocol.promptParams(sessionID: sessionID, text: text)
        )
    }

    func cancel() {
        guard let sessionID else { return }
        notify(
            "session/cancel",
            params: ClaudeACPProtocol.cancelParams(sessionID: sessionID)
        )
    }

    func stop() {
        intentionalShutdown = true
        output?.readabilityHandler = nil
        errorOutput?.readabilityHandler = nil
        if process?.isRunning == true { process?.terminate() }
        failPending(with: .processExited(nil))
        process = nil
        input = nil
        output = nil
        errorOutput = nil
        sessionID = nil
        outputBuffer.removeAll(keepingCapacity: false)
        diagnostics = ""
    }

    private func launch(npx: URL, model: String?) throws {
        let process = Process()
        let stdin = Pipe()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = npx
        process.arguments = [
            "--yes",
            ClaudeACPProtocol.adapterPackage
        ]
        process.currentDirectoryURL = Self.sessionWorkingDirectory()
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = stderr
        var environment = ProcessInfo.processInfo.environment
        environment.removeValue(forKey: "CLAUDECODE")
        environment.removeValue(forKey: "FORCE_COLOR")
        environment["NO_COLOR"] = "1"
        if let model {
            environment["ANTHROPIC_MODEL"] = model
        } else {
            environment.removeValue(forKey: "ANTHROPIC_MODEL")
        }
        environment["PATH"] = Self.executablePath(npx: npx, environment: environment)
        process.environment = environment
        intentionalShutdown = false
        output = stdout.fileHandleForReading
        errorOutput = stderr.fileHandleForReading
        input = stdin.fileHandleForWriting
        self.process = process

        output?.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            Task { @MainActor [weak self] in self?.receive(data) }
        }
        errorOutput?.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            Task { @MainActor [weak self] in self?.receiveDiagnostic(data) }
        }
        process.terminationHandler = { [weak self] terminated in
            Task { @MainActor [weak self] in
                self?.processDidExit(terminated, status: terminated.terminationStatus)
            }
        }
        do {
            try process.run()
        } catch {
            stop()
            throw ClaudeACPError.processLaunch(error.localizedDescription)
        }
    }

    private func request(
        _ method: String,
        params: JSONValue,
        timeout: Duration? = .seconds(60)
    ) async throws -> JSONValue {
        guard process?.isRunning == true else {
            throw ClaudeACPError.notConnected
        }
        let id = nextID
        nextID += 1
        return try await withCheckedThrowingContinuation { continuation in
            pending[id] = PendingRequest(method: method, continuation: continuation)
            do {
                try write(.object([
                    "jsonrpc": .string("2.0"),
                    "id": .integer(id),
                    "method": .string(method),
                    "params": params
                ]))
            } catch {
                pending.removeValue(forKey: id)?
                    .continuation.resume(throwing: error)
                return
            }
            guard let timeout else { return }
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: timeout)
                guard let request = self?.pending.removeValue(forKey: id) else { return }
                request.continuation.resume(
                    throwing: ClaudeACPError.timeout(request.method)
                )
            }
        }
    }

    private func notify(_ method: String, params: JSONValue) {
        try? write(.object([
            "jsonrpc": .string("2.0"),
            "method": .string(method),
            "params": params
        ]))
    }

    private func respond(id: JSONValue, result: JSONValue) {
        try? write(.object([
            "jsonrpc": .string("2.0"),
            "id": id,
            "result": result
        ]))
    }

    private func respondUnsupported(id: JSONValue, method: String) {
        try? write(.object([
            "jsonrpc": .string("2.0"),
            "id": id,
            "error": .object([
                "code": .integer(-32601),
                "message": .string("method not handled: \(method)")
            ])
        ]))
    }

    private func write(_ value: JSONValue) throws {
        guard let input else { throw ClaudeACPError.notConnected }
        var data = try JSONEncoder().encode(value)
        data.append(0x0A)
        try input.write(contentsOf: data)
    }

    private func receive(_ data: Data) {
        guard !data.isEmpty else { return }
        outputBuffer.append(data)
        while let newline = outputBuffer.firstIndex(of: 0x0A) {
            let line = outputBuffer[..<newline]
            outputBuffer.removeSubrange(...newline)
            guard !line.isEmpty,
                  let frame = try? JSONDecoder().decode(JSONValue.self, from: line)
            else { continue }
            handle(frame)
        }
    }

    private func handle(_ frame: JSONValue) {
        guard let object = frame.object else { return }
        if let method = object["method"]?.string {
            let params = object["params"] ?? .object([:])
            if let id = object["id"] {
                if method == "session/request_permission",
                   let permission = ClaudeACPProtocol.permission(
                       rpcID: id,
                       params: params
                   ) {
                    let allow = permission.options.first {
                        $0.kind.hasPrefix("allow")
                    } ?? permission.options.first
                    respond(
                        id: permission.rpcID,
                        result: ClaudeACPProtocol.permissionResult(
                            optionID: allow?.id
                        )
                    )
                } else {
                    respondUnsupported(id: id, method: method)
                }
            } else if method == "session/update",
                      let event = ClaudeACPProtocol.event(from: params) {
                eventHandler?(event)
            }
            return
        }
        guard let id = object["id"]?.int,
              let request = pending.removeValue(forKey: Int64(id)) else { return }
        if let error = object["error"]?.object {
            request.continuation.resume(
                throwing: ClaudeACPError.rpc(
                    code: error["code"]?.int ?? -1,
                    message: error["message"]?.string ?? "unknown error"
                )
            )
        } else {
            request.continuation.resume(returning: object["result"] ?? .null)
        }
    }

    private func receiveDiagnostic(_ data: Data) {
        guard !data.isEmpty else { return }
        diagnostics += String(decoding: data, as: UTF8.self)
        if diagnostics.count > 4_000 {
            diagnostics = String(diagnostics.suffix(4_000))
        }
    }

    /// A `restart()` can launch a replacement process before the previous
    /// one's asynchronous termination callback arrives; comparing identity
    /// against the current `process` discards that stale callback instead of
    /// tearing down the session that has already replaced it.
    private func processDidExit(_ terminated: Process, status: Int32) {
        guard process === terminated else { return }
        output?.readabilityHandler = nil
        errorOutput?.readabilityHandler = nil
        let detail = diagnostics.trimmingCharacters(in: .whitespacesAndNewlines)
        let message = detail.isEmpty ? "status \(status)" : detail
        failPending(with: .processExited(message))
        process = nil
        input = nil
        output = nil
        errorOutput = nil
        sessionID = nil
        if !intentionalShutdown { eventHandler?(.exited(message)) }
    }

    private func failPending(with error: ClaudeACPError) {
        let requests = pending.values
        pending.removeAll()
        for request in requests {
            request.continuation.resume(throwing: error)
        }
    }
}
