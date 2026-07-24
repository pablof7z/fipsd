import Testing
@testable import FIPSDFeature

@Test func subFrameEventsAreAnimatedInExactOrder() throws {
    let events = try [
        schedulerEvent(id: 0, timeNS: 1_000_000),
        schedulerEvent(id: 1, timeNS: 2_000_000),
        schedulerEvent(id: 2, timeNS: 3_000_000)
    ].map { try #require($0) }
    let scheduler = EventAwarePlaybackScheduler(maximumAnimatedDueEvents: 8)

    let first = scheduler.nextUpdate(
        events: events,
        cursor: 0,
        virtualTimeNS: 0,
        wallDeltaNS: 16_000_000,
        speed: 1
    )
    let second = scheduler.nextUpdate(
        events: events,
        cursor: first.eventRange.upperBound,
        virtualTimeNS: first.throughNS,
        wallDeltaNS: 16_000_000,
        speed: 1
    )

    #expect(first.eventRange == 0..<1)
    #expect(first.throughNS == 1_000_000)
    #expect(first.mode == .orderedEvent)
    #expect(second.eventRange == 1..<2)
    #expect(second.throughNS == 2_000_000)
}

@Test func densePlaybackWindowIsExactlySummarized() throws {
    let events = try (0..<10).map {
        try #require(schedulerEvent(id: $0, timeNS: UInt64($0 + 1)))
    }
    let scheduler = EventAwarePlaybackScheduler(maximumAnimatedDueEvents: 8)

    let update = scheduler.nextUpdate(
        events: events,
        cursor: 0,
        virtualTimeNS: 0,
        wallDeltaNS: 16_000_000,
        speed: 1
    )
    let batch = DisplayProjectionBatch(
        events: Array(events[update.eventRange]),
        fromNS: 0,
        throughNS: update.throughNS,
        mode: update.mode,
        compressionReason: update.compressionReason
    )

    #expect(update.eventRange == 0..<10)
    #expect(update.mode == .exactSummary)
    #expect(update.compressionReason == .playbackWindowDensity)
    #expect(batch.eventIDs == events.map(\.id))
    #expect(batch.eventOrdinals == events.map(\.ordinal))
    #expect(batch.isCompressed)
}

@Test func sameTimestampBurstIsDistinguishedFromWindowDensity() throws {
    let events = try (0..<9).map {
        try #require(schedulerEvent(id: $0, timeNS: 5))
    }
    let update = EventAwarePlaybackScheduler(maximumAnimatedDueEvents: 8)
        .nextUpdate(
            events: events,
            cursor: 0,
            virtualTimeNS: 0,
            wallDeltaNS: 16_000_000,
            speed: 1
        )

    #expect(update.mode == .exactSummary)
    #expect(update.compressionReason == .sameTimestampBurst)
}

@Test func batchIdentifiesCausalEntriesFromOutsideTheSummary() throws {
    let events = try [
        schedulerEvent(id: 0, timeNS: 1),
        schedulerEvent(id: 1, timeNS: 2, causalParent: "event-0"),
        schedulerEvent(id: 2, timeNS: 3, causalParent: "prior-event")
    ].map { try #require($0) }
    let batch = DisplayProjectionBatch(
        events: events,
        fromNS: 0,
        throughNS: 3,
        mode: .exactSummary,
        compressionReason: .playbackWindowDensity
    )

    #expect(batch.initiatingEventIDs == ["event-0", "event-2"])
}

private func schedulerEvent(
    id: Int,
    timeNS: UInt64,
    causalParent: String? = nil
) -> SimulationEvent? {
    SimulationEvent(.object([
        "event_id": .string("event-\(id)"),
        "virtual_time_ns": .integer(Int64(timeNS)),
        "ordinal": .integer(Int64(id)),
        "kind": .string("test.event"),
        "causal_parent": causalParent.map(JSONValue.string) ?? .null,
        "data": .object([:])
    ]))
}
