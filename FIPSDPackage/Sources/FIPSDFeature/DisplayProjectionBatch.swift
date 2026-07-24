import Foundation

struct DisplayProjectionBatch: Equatable, Sendable {
    static let empty = DisplayProjectionBatch(
        fromNS: 0,
        throughNS: 0,
        eventIDs: [],
        eventKinds: [],
        eventTimesNS: [],
        eventOrdinals: [],
        causalParents: [],
        mode: .idle,
        compressionReason: nil
    )

    static func viewChange(at virtualTimeNS: UInt64) -> DisplayProjectionBatch {
        DisplayProjectionBatch(
            fromNS: virtualTimeNS,
            throughNS: virtualTimeNS,
            eventIDs: [],
            eventKinds: [],
            eventTimesNS: [],
            eventOrdinals: [],
            causalParents: [],
            mode: .viewChange,
            compressionReason: nil
        )
    }

    let fromNS: UInt64
    let throughNS: UInt64
    let eventIDs: [String]
    let eventKinds: [String]
    let eventTimesNS: [UInt64]
    let eventOrdinals: [UInt64]
    let causalParents: [String?]
    let mode: DisplayProjectionMode
    let compressionReason: DisplayCompressionReason?

    var count: Int { eventIDs.count }
    var isCompressed: Bool {
        mode == .exactSummary || (mode == .seekReplay && count > 1)
    }

    var initiatingEventIDs: [String] {
        let members = Set(eventIDs)
        return zip(eventIDs, causalParents).compactMap { id, parent in
            if let parent, members.contains(parent) { return nil }
            return id
        }
    }

    var label: String {
        switch mode {
        case .idle:
            "Waiting for the next ordered event"
        case .interpolation:
            "Virtual-time interpolation · no ordered event crossed"
        case .orderedEvent:
            "1 ordered event animated · ordinal \(eventOrdinals.first ?? 0)"
        case .exactSummary:
            "\(count.formatted()) ordered events exactly summarized"
        case .seekReplay:
            "\(count.formatted()) ordered events replayed by explicit seek"
        case .viewChange:
            "Explicit visualization projection change · no simulation event"
        }
    }

    init(
        events: [SimulationEvent],
        fromNS: UInt64,
        throughNS: UInt64,
        mode: DisplayProjectionMode,
        compressionReason: DisplayCompressionReason? = nil
    ) {
        self.init(
            fromNS: fromNS,
            throughNS: throughNS,
            eventIDs: events.map(\.id),
            eventKinds: events.map(\.kind),
            eventTimesNS: events.map(\.timeNS),
            eventOrdinals: events.map(\.ordinal),
            causalParents: events.map(\.causalParent),
            mode: mode,
            compressionReason: compressionReason
        )
    }

    private init(
        fromNS: UInt64,
        throughNS: UInt64,
        eventIDs: [String],
        eventKinds: [String],
        eventTimesNS: [UInt64],
        eventOrdinals: [UInt64],
        causalParents: [String?],
        mode: DisplayProjectionMode,
        compressionReason: DisplayCompressionReason?
    ) {
        self.fromNS = fromNS
        self.throughNS = throughNS
        self.eventIDs = eventIDs
        self.eventKinds = eventKinds
        self.eventTimesNS = eventTimesNS
        self.eventOrdinals = eventOrdinals
        self.causalParents = causalParents
        self.mode = mode
        self.compressionReason = compressionReason
    }
}
