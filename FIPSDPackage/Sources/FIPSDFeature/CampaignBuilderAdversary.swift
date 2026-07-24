extension CampaignBuilder {
    static func adversaryConfiguration(_ raw: CampaignConfiguration) -> [String: Any] {
        let identities = raw.sybilEvents.reduce(0) {
            $0 + min(max(1, $1.count), 100_000)
        }
        guard identities > 0 else {
            return ["mode": "none", "actions": [], "budgets": [:]]
        }
        return [
            "mode": "authenticated-protocol-valid",
            "actions": ["sybil-concentration"],
            "budgets": [
                "operations": identities,
                "identities": identities,
                "bytes": identities * 4_096,
                "compute_units": identities * 4,
                "wall_time": duration(seconds: 60)
            ]
        ]
    }
}
