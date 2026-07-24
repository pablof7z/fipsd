import SwiftUI

struct TopologyEditorControls: View {
    @Bindable var model: WorkbenchModel

    var body: some View {
        DisclosureGroup("Direct-link topology editor") {
            VStack(alignment: .leading, spacing: 9) {
                Button("Edit currently rendered links", systemImage: "pencil.and.list.clipboard") {
                    model.captureRenderedTopology()
                }
                .disabled(model.state.edges.isEmpty)
                if model.configuration.topology == "explicit" {
                    HStack {
                        TextField("From", value: $model.explicitEdgeFrom, format: .number)
                            .textFieldStyle(.roundedBorder).frame(width: 64)
                        Image(systemName: "arrow.left.and.right")
                        TextField("To", value: $model.explicitEdgeTo, format: .number)
                            .textFieldStyle(.roundedBorder).frame(width: 64)
                        Button("Add") { model.addExplicitEdge() }
                            .disabled(!model.canAddExplicitEdge)
                    }
                    HStack {
                        Text("\(model.configuration.explicitEdges.count) undirected links")
                            .font(.caption).foregroundStyle(.secondary)
                        Spacer()
                        Button("Connected chain") { model.resetExplicitTopologyToChain() }
                            .controlSize(.small)
                    }
                    ForEach(Array(model.configuration.explicitEdges.enumerated()), id: \.offset) {
                        index, edge in
                        HStack {
                            Text("#\(edge.from) ↔ #\(edge.to)").font(.caption.monospaced())
                            Spacer()
                            Button("Remove", systemImage: "minus.circle") {
                                model.removeExplicitEdge(at: index)
                            }
                            .labelStyle(.iconOnly)
                            .buttonStyle(.plain)
                        }
                    }
                    Text("All node IDs must remain connected. The engine rejects dangling, duplicate, self, or disconnected links.")
                        .font(.caption2).foregroundStyle(.secondary)
                } else {
                    Text("Run a generated topology, then convert its exact rendered links into an editable explicit graph.")
                        .font(.caption2).foregroundStyle(.secondary)
                }
            }
            .padding(.top, 6)
        }
    }
}
