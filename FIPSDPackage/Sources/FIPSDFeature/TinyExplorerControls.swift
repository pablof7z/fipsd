import SwiftUI

struct TinyExplorerControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Exhaustive tiny-state exploration") {
            VStack(alignment: .leading, spacing: 9) {
                Stepper(
                    "At most \(model.tinyMaximumNodes) nodes",
                    value: $model.tinyMaximumNodes, in: 2...8
                )
                Stepper(
                    "At most \(model.tinyMaximumActions) actions",
                    value: $model.tinyMaximumActions, in: 1...7
                )
                Button("Explore every action order", systemImage: "arrow.triangle.branch") {
                    model.runTinyExploration()
                }
                .accessibilityIdentifier("tinyExplorationButton")
                .disabled(model.isRunning)
                Text(model.tinyStatus).font(.caption2).foregroundStyle(.secondary)
                Text("Uses configured manual lifecycle/network/link actions. With none, it probes disappear/reappear ordering. Factorial bounds are enforced before execution.")
                    .font(.caption2).foregroundStyle(.secondary)
            }.padding(.top, 6)
        }
    }
}
