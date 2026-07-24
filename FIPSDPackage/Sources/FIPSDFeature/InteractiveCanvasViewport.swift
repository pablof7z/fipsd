import SwiftUI

struct CanvasViewportTransform: Equatable {
    static let minimumScale: CGFloat = 0.2
    static let maximumScale: CGFloat = 12

    var scale: CGFloat = 1
    var offset: CGSize = .zero

    mutating func pan(by translation: CGSize) {
        offset.width += translation.width
        offset.height += translation.height
    }

    mutating func zoom(by factor: CGFloat, around anchor: CGPoint, in size: CGSize) {
        guard factor.isFinite, factor > 0, size.width > 0, size.height > 0 else {
            return
        }
        let nextScale = min(
            Self.maximumScale,
            max(Self.minimumScale, scale * factor)
        )
        let ratio = nextScale / scale
        let center = CGPoint(x: size.width / 2, y: size.height / 2)
        offset = CGSize(
            width: (1 - ratio) * (anchor.x - center.x) + ratio * offset.width,
            height: (1 - ratio) * (anchor.y - center.y) + ratio * offset.height
        )
        scale = nextScale
    }

    mutating func reset() {
        scale = 1
        offset = .zero
    }
}

struct InteractiveCanvasViewport<Content: View>: View {
    @ViewBuilder let content: () -> Content

    @State private var transform = CanvasViewportTransform()
    @State private var previousMagnification: CGFloat = 1
    @GestureState private var dragTranslation: CGSize = .zero

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                content()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .scaleEffect(transform.scale)
                    .offset(effectiveOffset)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .clipped()
            .simultaneousGesture(dragGesture)
            .simultaneousGesture(magnifyGesture(in: geometry.size))
            .overlay(alignment: .bottomTrailing) {
                controls(in: geometry.size)
            }
        }
    }

    private var effectiveOffset: CGSize {
        CGSize(
            width: transform.offset.width + dragTranslation.width,
            height: transform.offset.height + dragTranslation.height
        )
    }

    private var dragGesture: some Gesture {
        DragGesture(minimumDistance: 3)
            .updating($dragTranslation) { value, translation, _ in
                translation = value.translation
            }
            .onEnded { value in
                transform.pan(by: value.translation)
            }
    }

    private func magnifyGesture(in size: CGSize) -> some Gesture {
        MagnifyGesture(minimumScaleDelta: 0.005)
            .onChanged { value in
                let incremental = value.magnification / previousMagnification
                let anchor = CGPoint(
                    x: value.startAnchor.x * size.width,
                    y: value.startAnchor.y * size.height
                )
                transform.zoom(by: incremental, around: anchor, in: size)
                previousMagnification = value.magnification
            }
            .onEnded { _ in
                previousMagnification = 1
            }
    }

    private func controls(in size: CGSize) -> some View {
        HStack(spacing: 4) {
            viewportButton(
                "Zoom out",
                systemImage: "minus",
                identifier: "zoomOutButton"
            ) {
                transform.zoom(by: 0.8, around: center(of: size), in: size)
            }
            Button {
                withAnimation(.snappy(duration: 0.2)) {
                    transform.reset()
                }
            } label: {
                Text(transform.scale, format: .percent.precision(.fractionLength(0)))
                    .monospacedDigit()
                    .frame(minWidth: 44)
            }
            .buttonStyle(.borderless)
            .help("Reset zoom and position")
            .accessibilityLabel("Reset zoom and position")
            .accessibilityIdentifier("resetViewportButton")
            viewportButton(
                "Zoom in",
                systemImage: "plus",
                identifier: "zoomInButton"
            ) {
                transform.zoom(by: 1.25, around: center(of: size), in: size)
            }
        }
        .padding(6)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 9))
        .padding(12)
    }

    private func viewportButton(
        _ label: String,
        systemImage: String,
        identifier: String,
        action: @escaping () -> Void
    ) -> some View {
        Button {
            withAnimation(.snappy(duration: 0.15)) { action() }
        } label: {
            Image(systemName: systemImage)
                .frame(width: 20, height: 20)
        }
        .buttonStyle(.borderless)
        .help(label)
        .accessibilityLabel(label)
        .accessibilityIdentifier(identifier)
    }

    private func center(of size: CGSize) -> CGPoint {
        CGPoint(x: size.width / 2, y: size.height / 2)
    }
}
