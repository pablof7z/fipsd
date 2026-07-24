import Foundation

struct ApplicationTransferState: Equatable, Sendable, Identifiable {
    let id: String
    let source: Int
    let destination: Int
    var path: [Int]
    let totalBytes: Int
    var offeredBytes: Int
    var deliveredBytes: Int
    var deliveredChunkIndices: Set<Int> = []
    var startedAtNS: UInt64
    var lastDeliveryNS: UInt64?

    var progress: Double {
        guard totalBytes > 0 else { return 0 }
        return min(1, Double(deliveredBytes) / Double(totalBytes))
    }

    var routeLabel: String {
        path.map { "#\($0)" }.joined(separator: " → ")
    }
}

extension SimulationState {
    mutating func applyApplicationTransferOffer(_ event: SimulationEvent) {
        guard let shape = event.data["shape"]?.object,
              shape["kind"]?.string == "application-transfer",
              let id = shape["transfer_id"]?.string,
              let source = event.data["source"]?.int,
              let destination = event.data["destination"]?.int,
              let total = shape["total_bytes"]?.int else { return }
        let path = event.data["path"]?.array?.compactMap(\.int) ?? []
        let end = shape["byte_end"]?.int ?? 0
        var transfer = applicationTransfers[id] ?? ApplicationTransferState(
            id: id,
            source: source,
            destination: destination,
            path: path,
            totalBytes: total,
            offeredBytes: 0,
            deliveredBytes: 0,
            startedAtNS: event.timeNS
        )
        transfer.offeredBytes = max(transfer.offeredBytes, end)
        transfer.path = path
        applicationTransfers[id] = transfer
    }

    mutating func applyApplicationTransferDelivery(_ event: SimulationEvent) {
        guard event.data["final"]?.bool == true,
              let shape = event.data["shape"]?.object,
              shape["kind"]?.string == "application-transfer",
              let id = shape["transfer_id"]?.string,
              let chunkIndex = shape["chunk_index"]?.int,
              var transfer = applicationTransfers[id] else { return }
        if transfer.deliveredChunkIndices.insert(chunkIndex).inserted {
            transfer.deliveredBytes += event.data["useful_bytes"]?.int ?? 0
        }
        transfer.lastDeliveryNS = event.timeNS
        applicationTransfers[id] = transfer
    }
}
