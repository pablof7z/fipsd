import SwiftUI

struct TimelineBar: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        VStack(spacing: 7) {
            Slider(
                value: Binding(
                    get: { Double(model.virtualTimeNS) },
                    set: { model.seek(to: UInt64(max(0, $0))) }
                ),
                in: 0...Double(max(1, model.durationNS))
            )
            HStack(spacing: 12) {
                Button("Back", systemImage: "backward.frame.fill") { model.stepBackward() }
                    .labelStyle(.iconOnly)
                Button(
                    model.isPlaying ? "Pause" : "Play",
                    systemImage: model.isPlaying ? "pause.fill" : "play.fill"
                ) { model.togglePlayback() }
                .labelStyle(.iconOnly)
                .accessibilityIdentifier("playbackButton")
                Button("Forward", systemImage: "forward.frame.fill") { model.stepForward() }
                    .labelStyle(.iconOnly)
                Text(model.timeLabel).monospacedDigit().frame(width: 80, alignment: .leading)
                Picker("Speed", selection: $model.speed) {
                    Text("0.1×").tag(0.1)
                    Text("0.25×").tag(0.25)
                    Text("1×").tag(1.0)
                    Text("4×").tag(4.0)
                    Text("20×").tag(20.0)
                }
                .frame(width: 90)
                Spacer()
                Text(model.state.lastEvent?.kind ?? "No event")
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                if model.isRunning { ProgressView().controlSize(.small) }
            }
            .font(.caption)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(.bar)
    }
}
