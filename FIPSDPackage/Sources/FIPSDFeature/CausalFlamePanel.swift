import SwiftUI

struct CausalFlamePanel: View {
    let flames: [CausalFlameSummary]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Causal flame graph").font(.headline)
            ForEach(flames.prefix(10)) { flame in
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        Text(flame.label).lineLimit(1)
                        Spacer()
                        Text("\(flame.eventCount) events")
                    }
                    .font(.caption)
                    GeometryReader { geometry in
                        HStack(spacing: 0) {
                            ForEach(flame.slices) { slice in
                                Rectangle()
                                    .fill(color(slice.label))
                                    .frame(
                                        width: geometry.size.width
                                            * CGFloat(slice.count)
                                            / CGFloat(max(1, flame.eventCount))
                                    )
                                    .help("\(slice.label): \(slice.count) events")
                            }
                        }
                        .clipShape(RoundedRectangle(cornerRadius: 3))
                    }
                    .frame(height: 8)
                }
            }
            Text("Each bar is one input and its recorded transitive event descendants.")
                .font(.caption2).foregroundStyle(.secondary)
        }
    }

    private func color(_ plane: String) -> Color {
        switch plane {
        case "tree": .pink
        case "Bloom": .cyan
        case "payload": .yellow
        case "lookup": .purple
        case "session": .green
        case "input": .orange
        default: .gray
        }
    }
}
