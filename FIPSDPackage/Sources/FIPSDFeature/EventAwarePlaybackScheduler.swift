import Foundation

enum DisplayProjectionMode: String, Codable, Equatable, Sendable {
    case idle
    case interpolation
    case orderedEvent = "ordered-event"
    case exactSummary = "exact-event-summary"
    case seekReplay = "seek-replay"
}

enum DisplayCompressionReason: String, Codable, Equatable, Sendable {
    case sameTimestampBurst = "same-timestamp-burst"
    case playbackWindowDensity = "playback-window-density"
    case explicitSeek = "explicit-seek"
}

struct PlaybackUpdate: Equatable, Sendable {
    let throughNS: UInt64
    let eventRange: Range<Int>
    let mode: DisplayProjectionMode
    let compressionReason: DisplayCompressionReason?
}

struct EventAwarePlaybackScheduler: Equatable, Sendable {
    let maximumAnimatedDueEvents: Int

    init(maximumAnimatedDueEvents: Int = 8) {
        self.maximumAnimatedDueEvents = max(1, maximumAnimatedDueEvents)
    }

    func nextUpdate(
        events: [SimulationEvent],
        cursor: Int,
        virtualTimeNS: UInt64,
        wallDeltaNS: UInt64,
        speed: Double
    ) -> PlaybackUpdate {
        let availableEnd = max(events.last?.timeNS ?? 0, virtualTimeNS)
        let scaled = Double(wallDeltaNS) * max(0, speed)
        let advance = scaled >= Double(UInt64.max)
            ? UInt64.max
            : UInt64(scaled)
        let wallTarget = min(
            virtualTimeNS.saturatingAdd(advance),
            availableEnd
        )
        guard cursor < events.count else {
            return PlaybackUpdate(
                throughNS: wallTarget,
                eventRange: cursor..<cursor,
                mode: wallTarget == virtualTimeNS ? .idle : .interpolation,
                compressionReason: nil
            )
        }

        guard events[cursor].timeNS <= wallTarget else {
            return PlaybackUpdate(
                throughNS: wallTarget,
                eventRange: cursor..<cursor,
                mode: wallTarget == virtualTimeNS ? .idle : .interpolation,
                compressionReason: nil
            )
        }
        let target = wallTarget
        var dueEnd = cursor
        while dueEnd < events.count, events[dueEnd].timeNS <= target {
            dueEnd += 1
        }
        let dueCount = dueEnd - cursor
        precondition(dueCount > 0)
        if dueCount > maximumAnimatedDueEvents {
            let firstTime = events[cursor].timeNS
            let oneTimestamp = events[cursor..<dueEnd].allSatisfy {
                $0.timeNS == firstTime
            }
            return PlaybackUpdate(
                throughNS: target,
                eventRange: cursor..<dueEnd,
                mode: .exactSummary,
                compressionReason: oneTimestamp
                    ? .sameTimestampBurst
                    : .playbackWindowDensity
            )
        }
        return PlaybackUpdate(
            throughNS: max(virtualTimeNS, events[cursor].timeNS),
            eventRange: cursor..<(cursor + 1),
            mode: .orderedEvent,
            compressionReason: nil
        )
    }
}
