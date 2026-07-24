import Foundation

struct DisplayProjectionBatch: Equatable, Sendable {
    static let empty = DisplayProjectionBatch(
        fromNS: 0,
        throughNS: 0,
        eventIDs: [],
        eventKinds: []
    )

    let fromNS: UInt64
    let throughNS: UInt64
    let eventIDs: [String]
    let eventKinds: [String]

    var count: Int { eventIDs.count }
    var isCompressed: Bool { count > 1 }

    var label: String {
        switch count {
        case 0: "No ordered events in this display update"
        case 1: "1 ordered event in this display update"
        default: "\(count.formatted()) ordered events compressed in this display update"
        }
    }

    init(events: [SimulationEvent], fromNS: UInt64, throughNS: UInt64) {
        self.init(
            fromNS: fromNS,
            throughNS: throughNS,
            eventIDs: events.map(\.id),
            eventKinds: events.map(\.kind)
        )
    }

    private init(
        fromNS: UInt64,
        throughNS: UInt64,
        eventIDs: [String],
        eventKinds: [String]
    ) {
        self.fromNS = fromNS
        self.throughNS = throughNS
        self.eventIDs = eventIDs
        self.eventKinds = eventKinds
    }
}
