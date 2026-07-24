import Foundation

@MainActor
extension WorkbenchModel {
    func togglePlayback() {
        isPlaying.toggle()
        if isPlaying { startPlaybackLoop() }
    }

    func stepForward() {
        guard cursor < events.count else { return }
        let event = events[cursor]
        let priorTime = virtualTimeNS
        apply(event)
        displayProjectionBatch = DisplayProjectionBatch(
            events: [event],
            fromNS: priorTime,
            throughNS: virtualTimeNS
        )
        cursor += 1
    }

    func stepBackward() {
        guard cursor > 0 else { return }
        seek(to: events[cursor - 1].timeNS.saturatingSubtract(1))
    }

    func seek(to timeNS: UInt64) {
        isPlaying = false
        state = SimulationState()
        cursor = 0
        virtualTimeNS = timeNS
        var applied: [SimulationEvent] = []
        while cursor < events.count, events[cursor].timeNS <= timeNS {
            state.apply(events[cursor])
            applied.append(events[cursor])
            cursor += 1
        }
        state.expireTransmissions(at: timeNS)
        displayProjectionBatch = DisplayProjectionBatch(
            events: applied,
            fromNS: 0,
            throughNS: timeNS
        )
    }

    var durationNS: UInt64 { events.last?.timeNS ?? 0 }

    var timeLabel: String {
        String(format: "%.3f s", Double(virtualTimeNS) / 1e9)
    }

    func startPlaybackLoop() {
        guard playbackTask == nil || playbackTask?.isCancelled == true else { return }
        playbackTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(16))
                guard let self, self.isPlaying else { continue }
                self.advance(byWallNanoseconds: 16_000_000)
            }
        }
    }

    private func advance(byWallNanoseconds delta: UInt64) {
        let priorTime = virtualTimeNS
        let availableEnd = max(events.last?.timeNS ?? 0, virtualTimeNS)
        let advance = UInt64(Double(delta) * speed)
        virtualTimeNS = min(virtualTimeNS.saturatingAdd(advance), availableEnd)
        var applied: [SimulationEvent] = []
        while cursor < events.count, events[cursor].timeNS <= virtualTimeNS {
            let event = events[cursor]
            apply(event)
            applied.append(event)
            cursor += 1
        }
        state.expireTransmissions(at: virtualTimeNS)
        displayProjectionBatch = DisplayProjectionBatch(
            events: applied,
            fromNS: priorTime,
            throughNS: virtualTimeNS
        )
        if streamComplete, cursor == events.count, virtualTimeNS >= availableEnd {
            isPlaying = false
        }
    }
}
