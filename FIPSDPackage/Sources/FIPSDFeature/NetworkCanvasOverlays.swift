import SwiftUI

extension NetworkCanvas {
    @ViewBuilder
    var transferProgress: some View {
        if !state.applicationTransfers.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(state.applicationTransfers.values.sorted { $0.id < $1.id }.prefix(3)) {
                    transfer in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Label(transfer.id, systemImage: "arrow.down.doc.fill")
                            Spacer()
                            Text(
                                transfer.progress,
                                format: .percent.precision(.fractionLength(1))
                            )
                        }
                        ProgressView(value: transfer.progress)
                            .tint(.yellow)
                        Text(
                            "\(transfer.routeLabel) · "
                                + "\(bytes(transfer.deliveredBytes)) / "
                                + "\(bytes(transfer.totalBytes))"
                        )
                        .foregroundStyle(.secondary)
                    }
                }
            }
            .font(.caption)
            .padding(10)
            .frame(width: 310)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
            .padding(12)
        }
    }

    private func bytes(_ value: Int) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(value), countStyle: .file)
    }
}
