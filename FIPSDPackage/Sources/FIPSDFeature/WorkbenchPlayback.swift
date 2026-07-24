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
        state.expireTransmissions(at: virtualTimeNS)
        publishRenderFrame(DisplayProjectionBatch(
            events: [event],
            fromNS: priorTime,
            throughNS: virtualTimeNS,
            mode: .orderedEvent
        ))
        cursor += 1
        if streamComplete, cursor == events.count {
            finalizeRendererEvidence()
        }
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
        publishRenderFrame(DisplayProjectionBatch(
            events: applied,
            fromNS: 0,
            throughNS: timeNS,
            mode: .seekReplay,
            compressionReason: applied.count > 1 ? .explicitSeek : nil
        ))
        if streamComplete, cursor == events.count {
            finalizeRendererEvidence()
        }
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

    func advance(byWallNanoseconds delta: UInt64) {
        let priorTime = virtualTimeNS
        let update = EventAwarePlaybackScheduler().nextUpdate(
            events: events,
            cursor: cursor,
            virtualTimeNS: virtualTimeNS,
            wallDeltaNS: delta,
            speed: speed
        )
        let applied = Array(events[update.eventRange])
        for event in applied {
            apply(event)
        }
        cursor = update.eventRange.upperBound
        virtualTimeNS = max(virtualTimeNS, update.throughNS)
        state.expireTransmissions(at: virtualTimeNS)
        let batch = DisplayProjectionBatch(
            events: applied,
            fromNS: priorTime,
            throughNS: virtualTimeNS,
            mode: update.mode,
            compressionReason: update.compressionReason
        )
        if update.mode == .idle {
            displayProjectionBatch = batch
        } else {
            publishRenderFrame(batch)
        }
        let availableEnd = max(events.last?.timeNS ?? 0, virtualTimeNS)
        if streamComplete, cursor == events.count, virtualTimeNS >= availableEnd {
            isPlaying = false
            finalizeRendererEvidence()
        }
    }
}
