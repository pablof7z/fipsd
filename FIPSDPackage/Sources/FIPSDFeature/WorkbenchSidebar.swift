import SwiftUI

struct WorkbenchSidebar: View {
    @Bindable var model: WorkbenchModel
    @Bindable var agentModel: ClaudeAgentModel
    let openArtifact: () -> Void

    @State private var selection = SidebarSelection.agent

    var body: some View {
        VStack(spacing: 0) {
            Picker("Sidebar", selection: $selection) {
                ForEach(SidebarSelection.allCases) { item in
                    Text(item.rawValue).tag(item)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(12)
            .accessibilityIdentifier("workbench-sidebar-picker")

            Divider()

            switch selection {
            case .experiment:
                ControlPanel(model: model, openArtifact: openArtifact)
            case .agent:
                ClaudeAgentSidebar(model: agentModel)
            }
        }
        .frame(minWidth: 290, idealWidth: 330, maxWidth: 380)
        .background(.thinMaterial)
    }
}

private enum SidebarSelection: String, CaseIterable, Identifiable {
    case experiment = "Experiment"
    case agent = "Agent"

    var id: Self { self }
}
