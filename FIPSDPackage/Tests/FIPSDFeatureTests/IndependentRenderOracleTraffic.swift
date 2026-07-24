@testable import FIPSDFeature

extension IndependentRenderOracle {
    mutating func applyDue(
        _ eventID: String,
        data: [String: JSONValue],
        timeNS: UInt64,
        plane: String
    ) {
        guard let from = data["from"]?.int,
              let to = data["to"]?.int else { return }
        for value in data["deliveries"]?.array ?? [] {
            guard let delivery = value.object,
                  let endNS = delivery["deliver_at_ns"]?.uint64 else {
                continue
            }
            let copy = delivery["copy"]?.int ?? 0
            let id = "\(eventID):\(copy)"
            flights[id] = OracleFlight(
                id: id, from: from, to: to, startNS: timeNS,
                endNS: endNS, plane: plane
            )
        }
    }

    mutating func removeFlight(
        data: [String: JSONValue],
        causalParent: String?
    ) {
        guard let from = data["from"]?.int,
              let to = data["to"]?.int else { return }
        let copy = data["copy"]?.int ?? 0
        if let causalParent {
            flights.removeValue(forKey: "\(causalParent):\(copy)")
        } else {
            flights = flights.filter {
                !($0.value.from == from
                    && $0.value.to == to
                    && $0.value.id.hasSuffix(":\(copy)"))
            }
        }
    }

    mutating func applyTransferOffer(
        _ data: [String: JSONValue],
        timeNS: UInt64
    ) {
        guard let shape = data["shape"]?.object,
              shape["kind"]?.string == "application-transfer",
              let id = shape["transfer_id"]?.string,
              let source = data["source"]?.int,
              let destination = data["destination"]?.int,
              let total = shape["total_bytes"]?.int else { return }
        let path = data["path"]?.array?.compactMap(\.int) ?? []
        let end = shape["byte_end"]?.int ?? 0
        var transfer = transfers[id] ?? OracleTransfer(
            id: id, source: source, destination: destination,
            path: path, totalBytes: total, offeredBytes: 0,
            deliveredBytes: 0, deliveredChunks: [],
            startedAtNS: timeNS, lastDeliveryNS: nil
        )
        transfer.offeredBytes = max(transfer.offeredBytes, end)
        transfer.path = path
        transfers[id] = transfer
    }

    mutating func applyTransferDelivery(
        _ data: [String: JSONValue],
        timeNS: UInt64
    ) {
        guard data["final"]?.bool == true,
              let shape = data["shape"]?.object,
              shape["kind"]?.string == "application-transfer",
              let id = shape["transfer_id"]?.string,
              let chunk = shape["chunk_index"]?.int,
              var transfer = transfers[id] else { return }
        if transfer.deliveredChunks.insert(chunk).inserted {
            transfer.deliveredBytes += data["useful_bytes"]?.int ?? 0
        }
        transfer.lastDeliveryNS = timeNS
        transfers[id] = transfer
    }

    static func anomalyNodeIDs(_ values: [JSONValue]) -> Set<Int> {
        struct Load {
            let from: Int
            let to: Int
            var bytes: UInt64
            var id: String { "\(from)-\(to)" }
        }
        var loads: [String: Load] = [:]
        for value in values {
            guard let event = value.object,
                  let kind = event["kind"]?.string,
                  kind.hasSuffix(".due") || kind.hasSuffix("-due"),
                  let data = event["data"]?.object,
                  let from = data["from"]?.int,
                  let to = data["to"]?.int else { continue }
            let key = "\(from)-\(to)"
            var load = loads[key] ?? Load(from: from, to: to, bytes: 0)
            load.bytes += data["transport_bytes"]?.uint64
                ?? data["frame_bytes"]?.uint64 ?? 0
            loads[key] = load
        }
        let top = loads.values.sorted {
            $0.bytes == $1.bytes ? $0.id < $1.id : $0.bytes > $1.bytes
        }.prefix(12)
        return Set(top.flatMap { [$0.from, $0.to] })
    }
}
