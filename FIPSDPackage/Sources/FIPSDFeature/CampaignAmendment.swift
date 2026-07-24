import Foundation

enum CampaignAmendmentError: LocalizedError {
    case invalidDocument
    case unsupportedAction(String)
    case eventBeforeCursor(String)
    case duplicateEventID(String)

    var errorDescription: String? {
        switch self {
        case .invalidDocument: "The model returned an invalid scenario amendment."
        case let .unsupportedAction(action): "The model requested unsupported action \(action)."
        case let .eventBeforeCursor(id): "Amendment event \(id) attempts to rewrite rendered history."
        case let .duplicateEventID(id): "Amendment event ID \(id) already exists."
        }
    }
}

struct CampaignAmendment {
    private static let actions: Set<String> = [
        "set-link-conditions", "restore-link-conditions", "introduce-lower-root-node",
        "introduce-node", "disappear-node", "reappear-node", "partition-network", "merge-network",
        "fail-transport-class", "restore-transport-class", "synchronized-session-rekey",
        "expire-coordinate-cache", "simultaneous-lookups", "swap-parent-ancestry",
        "alternate-parent-quality", "attach-authenticated-sybils"
    ]

    static func applying(
        _ amendmentData: Data,
        to campaignData: Data,
        noEarlierThan minimumNS: UInt64,
        realizedArrivals: Int,
        attachmentNode: Int? = nil
    ) throws -> Data {
        guard let amendment = try JSONSerialization.jsonObject(with: amendmentData)
                as? [String: Any],
              var campaign = try JSONSerialization.jsonObject(with: campaignData)
                as? [String: Any] else {
            throw CampaignAmendmentError.invalidDocument
        }
        var existing = campaign["events"] as? [[String: Any]] ?? []
        let previousManualArrivals = nodeArrivalCount(existing)
        if amendment["stop_scheduled_arrivals"] as? Bool == true,
           var identities = campaign["identities"] as? [String: Any],
           var arrivals = identities["arrivals"] as? [String: Any] {
            arrivals["count"] = realizedArrivals
            identities["arrivals"] = arrivals
            campaign["identities"] = identities
        }
        let cancelled = Set(amendment["cancel_future_event_ids"] as? [String] ?? [])
        existing.removeAll { event in
            guard let id = event["id"] as? String, cancelled.contains(id) else { return false }
            return eventTimeNS(event) >= minimumNS
        }
        var usedIDs = Set(existing.compactMap { $0["id"] as? String })
        let additions = amendment["events"] as? [[String: Any]] ?? []
        var newAttachmentTargets: [[Int]] = []
        guard additions.count <= 10_000 else { throw CampaignAmendmentError.invalidDocument }
        for rawEvent in additions {
            var event = rawEvent
            guard let id = event["id"] as? String,
                  let action = event["action"] as? String else {
                throw CampaignAmendmentError.invalidDocument
            }
            guard actions.contains(action) else {
                throw CampaignAmendmentError.unsupportedAction(action)
            }
            guard !usedIDs.contains(id) else {
                throw CampaignAmendmentError.duplicateEventID(id)
            }
            guard eventTimeNS(event) >= minimumNS else {
                throw CampaignAmendmentError.eventBeforeCursor(id)
            }
            if action == "introduce-lower-root-node" {
                let target = event["target"] as? Int ?? attachmentNode ?? 0
                event["target"] = target
                newAttachmentTargets.append([target])
            } else if action == "introduce-node" {
                var parameters = event["parameters"] as? [String: Any] ?? [:]
                var targets = parameters["attachments"] as? [Int] ?? []
                if targets.isEmpty { targets = [attachmentNode ?? 0] }
                parameters["attachments"] = Array(Set(targets)).sorted()
                event["parameters"] = parameters
                newAttachmentTargets.append(targets)
            }
            usedIDs.insert(id)
            existing.append(event)
        }
        campaign["events"] = existing
        CampaignIdentityBudget.adjustScale(
            in: &campaign,
            previousManualArrivals: previousManualArrivals,
            currentManualArrivals: nodeArrivalCount(existing),
            newAttachmentTargets: newAttachmentTargets
        )
        CampaignIdentityBudget.reserveManualArrivals(in: &campaign)
        return try JSONSerialization.data(
            withJSONObject: campaign,
            options: [.prettyPrinted, .sortedKeys]
        )
    }

    private static func nodeArrivalCount(_ events: [[String: Any]]) -> Int {
        events.filter {
            ["introduce-lower-root-node", "introduce-node"].contains(
                $0["action"] as? String
            )
        }.count
    }

    private static func eventTimeNS(_ event: [String: Any]) -> UInt64 {
        if let value = event["at"] as? [String: Any],
           let nanoseconds = value["nanoseconds"] as? NSNumber {
            return nanoseconds.uint64Value
        }
        guard let raw = event["at"] as? String else { return 0 }
        let units: [(String, Double)] = [
            ("ns", 1), ("us", 1_000), ("ms", 1_000_000), ("s", 1_000_000_000)
        ]
        for (suffix, multiplier) in units where raw.hasSuffix(suffix) {
            let amount = String(raw.dropLast(suffix.count))
            if let number = Double(amount), number >= 0 {
                return UInt64((number * multiplier).rounded())
            }
        }
        return 0
    }
}
