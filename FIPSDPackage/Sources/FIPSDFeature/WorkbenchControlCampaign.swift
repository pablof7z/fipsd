import Foundation

extension WorkbenchModel {
    func runControlCampaign(_ arguments: [String: JSONValue]) throws -> JSONValue {
        guard let campaign = arguments["campaign"]?.object else {
            throw AppControlError.invalidArgument("campaign")
        }
        let raw = try JSONEncoder().encode(JSONValue.object(campaign))
        let authored = try annotated(raw, author: "mcp")
        generatedSpec = String(decoding: authored, as: UTF8.self)
        startRun(
            campaign: authored,
            author: "mcp",
            authoringPrompt: arguments["description"]?.string
        )
        return controlSnapshot(limit: 20)
    }

    func injectControlEvent(_ arguments: [String: JSONValue]) throws -> JSONValue {
        guard let campaign = activeCampaign else {
            throw AppControlError.noActiveCampaign
        }
        guard let action = arguments["action"]?.string, !action.isEmpty else {
            throw AppControlError.invalidArgument("action")
        }
        let minimum = virtualTimeNS.saturatingAdd(1)
        let atNS: UInt64
        if let explicit = arguments["at_ns"]?.uint64 {
            atNS = max(explicit, minimum)
        } else {
            let delayMS = max(0, arguments["after_ms"]?.double ?? 100)
            atNS = virtualTimeNS.saturatingAdd(
                UInt64(min(delayMS * 1_000_000, Double(UInt64.max)))
            )
        }
        var event: [String: Any] = [
            "id": arguments["id"]?.string ?? "mcp-\(action)-\(atNS)",
            "at": "\(atNS)ns",
            "action": action
        ]
        if let target = arguments["target"] {
            event["target"] = target.foundationValue
        }
        if let parameters = arguments["parameters"]?.object {
            event["parameters"] = parameters.mapValues(\.foundationValue)
        }
        let amendment = try JSONSerialization.data(
            withJSONObject: ["events": [event]],
            options: [.sortedKeys]
        )
        let context = try LiveRunContext.make(
            state: state,
            events: events,
            cursor: cursor,
            timeNS: virtualTimeNS,
            campaign: campaign
        )
        let amended = try CampaignAmendment.applying(
            amendment,
            to: campaign,
            noEarlierThan: minimum,
            realizedArrivals: context.realizedArrivals,
            attachmentNode: context.currentRootID
        )
        let authored = try annotated(amended, author: "mcp")
        generatedSpec = String(decoding: authored, as: UTF8.self)
        let resume = virtualTimeNS
        startRun(
            campaign: authored,
            author: "mcp",
            authoringPrompt: "Injected \(action)",
            resumeAtNS: resume,
            authoringContext: context.data
        )
        return controlSnapshot(limit: 20)
    }
}
