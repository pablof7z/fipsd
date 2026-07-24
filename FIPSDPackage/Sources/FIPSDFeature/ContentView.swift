import SwiftUI
import UniformTypeIdentifiers

public struct ContentView: View {
    @State private var model = WorkbenchModel()
    @State private var agentModel = ClaudeAgentModel()
    @State private var artifactImporterPresented = false
    @State private var inspectorPresented = false

    public var body: some View {
        HSplitView {
            WorkbenchSidebar(
                model: model,
                agentModel: agentModel
            ) {
                artifactImporterPresented = true
            }
            VStack(spacing: 0) {
                NetworkCanvas(
                    frame: model.renderFrame,
                    state: model.state,
                    selection: $model.selectedNodeID,
                    mode: $model.visualizationMode,
                    cohortState: model.cohortState,
                    anomalyNodeIDs: model.analysis.anomalyNodeIDs,
                    displayBatch: model.displayProjectionBatch,
                    sourceFidelity: model.summary.fidelity
                )
                TimelineBar(model: model)
            }
            .frame(minWidth: 560, minHeight: 500)
            if inspectorPresented {
                InspectorPanel(model: model)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .frame(minWidth: 1_150, minHeight: 720)
        .toolbar {
            ToolbarItem {
                Button {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        inspectorPresented.toggle()
                    }
                } label: {
                    Image(systemName: "sidebar.right")
                }
                .help(inspectorPresented ? "Hide inspector" : "Show inspector")
                .accessibilityLabel(
                    inspectorPresented ? "Hide inspector" : "Show inspector"
                )
                .accessibilityIdentifier("toggleRightInspectorButton")
                .keyboardShortcut("i", modifiers: [.command, .option])
            }
        }
        .fileImporter(
            isPresented: $artifactImporterPresented,
            allowedContentTypes: [.json],
            allowsMultipleSelection: false
        ) { result in
            if case let .success(urls) = result, let url = urls.first {
                model.loadArtifact(url)
            }
        }
        .task {
            model.startControlServer()
            await agentModel.connect()
        }
        .onDisappear { agentModel.stop() }
    }

    public init() {}
}
