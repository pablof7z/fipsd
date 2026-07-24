import Foundation

extension WorkbenchModel {
    func startControlServer() {
        guard controlServer == nil else { return }
        do {
            let server = AppControlServer(model: self)
            try server.start()
            controlServer = server
        } catch {
            status = "App control endpoint failed: \(error.localizedDescription)"
        }
    }

    func handleControl(_ request: AppControlRequest) -> AppControlResponse {
        do {
            let result = try performControl(
                request.command,
                arguments: request.arguments
            )
            return .success(request.id, result)
        } catch {
            return .failure(request.id, error.localizedDescription)
        }
    }

    private func performControl(
        _ command: String,
        arguments: [String: JSONValue]
    ) throws -> JSONValue {
        switch command {
        case "get_state":
            return controlSnapshot(limit: arguments["limit"]?.int ?? 100)
        case "start_experiment":
            try selectProvider(arguments["model"]?.string)
            guard let text = arguments["prompt"]?.string,
                  !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw AppControlError.invalidArgument("prompt")
            }
            prompt = text
            generateAndRun()
            return controlSnapshot(limit: 20)
        case "amend_experiment":
            try selectProvider(arguments["model"]?.string)
            guard activeCampaign != nil else { throw AppControlError.noActiveCampaign }
            guard let text = arguments["prompt"]?.string,
                  !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw AppControlError.invalidArgument("prompt")
            }
            prompt = text
            amendCurrentRunFromPrompt()
            return controlSnapshot(limit: 20)
        case "playback":
            try applyPlaybackControl(arguments)
            return controlSnapshot(limit: 20)
        case "set_parameters":
            try applyControlParameters(arguments["parameters"]?.object ?? [:])
            if arguments["run"]?.bool == true { runConfigured() }
            return controlSnapshot(limit: 20)
        case "run_campaign":
            return try runControlCampaign(arguments)
        case "inject_event":
            return try injectControlEvent(arguments)
        case "save_experiment":
            return try saveControlExperiment(arguments)
        case "list_experiments":
            return try listControlExperiments()
        case "rerun_experiment":
            return try rerunControlExperiment(arguments)
        case "get_analysis":
            return controlAnalysis()
        case "explain":
            return controlExplanation(focus: arguments["focus"]?.string)
        default:
            throw AppControlError.unsupportedCommand(command)
        }
    }

    private func selectProvider(_ requested: String?) throws {
        guard let requested else { return }
        switch requested.lowercased().replacingOccurrences(of: "_", with: "-") {
        case "auto", "automatic": provider = .automatic
        case "sonnet", "claude-sonnet": provider = .claudeSonnet
        case "haiku", "claude-haiku": provider = .claudeHaiku
        case "opus", "claude-opus": provider = .claudeOpus
        case "codex": provider = .codex
        default: throw AppControlError.invalidArgument("model")
        }
    }

    private func applyPlaybackControl(_ arguments: [String: JSONValue]) throws {
        guard let action = arguments["action"]?.string else {
            throw AppControlError.invalidArgument("action")
        }
        switch action {
        case "play":
            isPlaying = true
            startPlaybackLoop()
        case "pause":
            isPlaying = false
        case "toggle":
            togglePlayback()
        case "stop":
            stopControlledExperiment()
        case "step_forward":
            stepForward()
        case "step_backward":
            stepBackward()
        case "seek":
            let time = arguments["time_ns"]?.uint64
                ?? arguments["time_seconds"]?.double.map { UInt64(max(0, $0) * 1e9) }
            guard let time else { throw AppControlError.invalidArgument("time_ns") }
            seek(to: time)
        case "set_speed":
            guard let value = arguments["speed"]?.double,
                  (0.01...1_000).contains(value) else {
                throw AppControlError.invalidArgument("speed")
            }
            speed = value
        default:
            throw AppControlError.invalidArgument("action")
        }
    }

    private func stopControlledExperiment() {
        runTask?.cancel()
        playbackTask?.cancel()
        runTask = nil
        playbackTask = nil
        isRunning = false
        isPlaying = false
        status = "Experiment stopped by MCP control."
    }
}
