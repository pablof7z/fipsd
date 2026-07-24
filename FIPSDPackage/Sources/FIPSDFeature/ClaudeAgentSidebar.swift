import SwiftUI
import MarkdownUI

struct ClaudeAgentSidebar: View {
    @Bindable var model: ClaudeAgentModel

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            transcript
            Divider()
            composer
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "sparkles")
                    .foregroundStyle(.purple)
                Text("Claude")
                    .font(.headline)
                Spacer()
                status
                Button {
                    model.restart()
                } label: {
                    Image(systemName: "plus.bubble")
                }
                .buttonStyle(.plain)
                .help("New conversation")
                .accessibilityLabel("New Claude conversation")
            }
            HStack(spacing: 6) {
                Image(systemName: "point.3.connected.trianglepath.dotted")
                Text("Wind Tunnel MCP")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(14)
    }

    private var status: some View {
        HStack(spacing: 5) {
            Circle()
                .fill(statusColor)
                .frame(width: 7, height: 7)
            Text(model.state.label)
        }
        .font(.caption)
        .foregroundStyle(.secondary)
    }

    private var statusColor: Color {
        switch model.state {
        case .disconnected: .secondary
        case .connecting: .blue
        case .ready: .green
        case .responding: .orange
        case .failed: .red
        }
    }

    private var transcript: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    ForEach(model.transcript) { entry in
                        transcriptRow(entry)
                            .id(entry.id)
                    }
                }
                .padding(14)
            }
            .onChange(of: model.transcript.count) {
                guard let id = model.transcript.last?.id else { return }
                withAnimation { proxy.scrollTo(id, anchor: .bottom) }
            }
        }
    }

    @ViewBuilder
    private func transcriptRow(_ entry: ClaudeTranscriptEntry) -> some View {
        switch entry.role {
        case .user:
            Text(entry.text)
                .textSelection(.enabled)
                .padding(.horizontal, 11)
                .padding(.vertical, 8)
                .background(Color.accentColor.opacity(0.14), in: RoundedRectangle(cornerRadius: 10))
                .frame(maxWidth: .infinity, alignment: .trailing)
        case .agent:
            Markdown(entry.text)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .activity:
            activity(entry)
        case .notice:
            Label(entry.text, systemImage: "info.circle")
                .font(.caption)
                .foregroundStyle(.secondary)
        case .failure:
            Label(entry.text, systemImage: "exclamationmark.triangle")
                .font(.caption)
                .foregroundStyle(.red)
        }
    }

    private func activity(_ entry: ClaudeTranscriptEntry) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 7) {
            if entry.detail == "completed" {
                Image(systemName: "checkmark.circle")
                    .foregroundStyle(.green)
            } else if entry.detail == "failed" {
                Image(systemName: "xmark.circle")
                    .foregroundStyle(.red)
            } else {
                ProgressView()
                    .controlSize(.small)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(entry.text)
                    .lineLimit(2)
                if let detail = entry.detail {
                    Text(detail.capitalized)
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 9) {
            HStack(alignment: .bottom, spacing: 8) {
                TextField(
                    "Ask Claude about the experiment…",
                    text: $model.draft,
                    axis: .vertical
                )
                .textFieldStyle(.plain)
                .lineLimit(1...5)
                .onSubmit { model.send() }
                .disabled(!model.state.isConnected || model.state == .responding)
                .accessibilityIdentifier("claude-agent-composer")

                if model.state == .responding {
                    Button {
                        model.cancel()
                    } label: {
                        Image(systemName: "stop.fill")
                    }
                    .buttonStyle(.bordered)
                    .help("Stop Claude")
                } else {
                    Button {
                        model.send()
                    } label: {
                        Image(systemName: "arrow.up")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!model.canSend)
                    .help("Send")
                    .accessibilityIdentifier("claude-agent-send")
                }
            }
            if case let .failed(message) = model.state {
                HStack {
                    Text(message)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                    Spacer()
                    Button("Retry") {
                        model.restart()
                    }
                    .font(.caption)
                }
            } else if let usage = model.usageLabel {
                Text(usage)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(14)
    }
}
