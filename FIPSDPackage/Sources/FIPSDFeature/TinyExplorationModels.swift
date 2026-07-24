import Foundation

struct TinyCounterexampleSummary: Identifiable, Equatable, Sendable {
    let id: String
    let order: [String]
    let failure: String
}

struct TinyExplorationSummary: Equatable, Sendable {
    var explorationID = ""
    var fidelity = ""
    var actionCount = 0
    var expected = 0
    var explored = 0
    var exhaustive = false
    var terminalStates = 0
    var counterexamples: [TinyCounterexampleSummary] = []

    static func parse(_ root: [String: JSONValue]) -> TinyExplorationSummary {
        TinyExplorationSummary(
            explorationID: root["exploration_id"]?.string ?? "",
            fidelity: root["fidelity"]?.string ?? "",
            actionCount: root["action_count"]?.int ?? 0,
            expected: root["expected_permutations"]?.int ?? 0,
            explored: root["explored_permutations"]?.int ?? 0,
            exhaustive: root["exhaustive"]?.bool ?? false,
            terminalStates: root["terminal_states"]?.object?.count ?? 0,
            counterexamples: (root["counterexamples"]?.array ?? []).compactMap { value in
                guard let object = value.object,
                      let id = object["id"]?.string,
                      let failure = object["failure"]?.string else { return nil }
                return TinyCounterexampleSummary(
                    id: id,
                    order: (object["action_order"]?.array ?? []).compactMap(\.string),
                    failure: failure
                )
            }
        )
    }
}
