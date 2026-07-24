import Foundation
import Observation

@MainActor
@Observable
final class ClaudeAgentModel {
    var state = ClaudeConnectionState.disconnected
    var draft = ""
    var transcript: [ClaudeTranscriptEntry] = [
        ClaudeTranscriptEntry(
            role: .notice,
            text: "Claude can inspect and control this workbench through the Wind Tunnel MCP."
        )
    ]
    var usageLabel: String?

    @ObservationIgnored private let client = ClaudeACPClient()
    @ObservationIgnored private var turnTask: Task<Void, Never>?
    @ObservationIgnored private var activeMessageID: String?

    var canSend: Bool {
        state == .ready
            && !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func connect() async {
        guard !state.isConnected, state != .connecting else { return }
        state = .connecting
        do {
            try await client.connect(
                cwd: ClaudeACPClient.sessionWorkingDirectory()
            ) { [weak self] event in
                self?.receive(event)
            }
            state = .ready
        } catch {
            state = .failed(error.localizedDescription)
            appendFailure(error.localizedDescription)
        }
    }

    func send() {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard state == .ready, !text.isEmpty else { return }
        draft = ""
        transcript.append(
            ClaudeTranscriptEntry(role: .user, text: text)
        )
        activeMessageID = "agent-\(UUID().uuidString)"
        state = .responding
        turnTask = Task {
            do {
                let reason = try await client.prompt(text)
                guard !Task.isCancelled else { return }
                state = .ready
                activeMessageID = nil
                if reason != "end_turn" {
                    transcript.append(
                        ClaudeTranscriptEntry(
                            role: .notice,
                            text: "Claude stopped: \(reason.replacingOccurrences(of: "_", with: " "))."
                        )
                    )
                }
            } catch {
                guard !Task.isCancelled else { return }
                state = .failed(error.localizedDescription)
                activeMessageID = nil
                appendFailure(error.localizedDescription)
            }
        }
    }

    func cancel() {
        guard state == .responding else { return }
        client.cancel()
    }

    func restart() {
        turnTask?.cancel()
        turnTask = nil
        client.stop()
        activeMessageID = nil
        state = .disconnected
        transcript = [
            ClaudeTranscriptEntry(
                role: .notice,
                text: "Starting a new Claude conversation with Wind Tunnel MCP access."
            )
        ]
        Task { await connect() }
    }

    func stop() {
        turnTask?.cancel()
        turnTask = nil
        client.stop()
        state = .disconnected
    }

    private func receive(_ event: ClaudeACPEvent) {
        switch event {
        case let .message(messageID, text):
            appendAgentText(id: messageID, text: text)
        case let .activity(id, title, status):
            updateActivity(id: id, title: title, status: status)
        case let .usage(used, size):
            usageLabel = size > 0
                ? "\(used.formatted()) / \(size.formatted()) tokens"
                : "\(used.formatted()) tokens"
        case let .diagnostic(message):
            transcript.append(
                ClaudeTranscriptEntry(role: .notice, text: message)
            )
        case let .exited(message):
            let detail = message ?? "The ACP process ended."
            state = .failed(detail)
            appendFailure(detail)
        }
    }

    private func appendAgentText(id messageID: String?, text: String) {
        let id = messageID.map { "agent-\($0)" } ?? activeMessageID
            ?? "agent-\(UUID().uuidString)"
        if let index = transcript.lastIndex(where: { $0.id == id }) {
            transcript[index].text += text
        } else {
            transcript.append(
                ClaudeTranscriptEntry(id: id, role: .agent, text: text)
            )
        }
    }

    private func updateActivity(id: String, title: String, status: String?) {
        let transcriptID = "tool-\(id)"
        let detail = status?.replacingOccurrences(of: "_", with: " ")
        if let index = transcript.lastIndex(where: { $0.id == transcriptID }) {
            if title != "Using a tool" { transcript[index].text = title }
            transcript[index].detail = detail
        } else {
            transcript.append(
                ClaudeTranscriptEntry(
                    id: transcriptID,
                    role: .activity,
                    text: title,
                    detail: detail
                )
            )
        }
    }

    private func appendFailure(_ message: String) {
        guard transcript.last?.role != .failure
                || transcript.last?.text != message else { return }
        transcript.append(
            ClaudeTranscriptEntry(role: .failure, text: message)
        )
    }
}
