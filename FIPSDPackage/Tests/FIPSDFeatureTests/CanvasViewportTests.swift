import CoreGraphics
import Testing
@testable import FIPSDFeature

@Test func viewportPansAndResets() {
    var viewport = CanvasViewportTransform()
    viewport.pan(by: CGSize(width: 80, height: -35))

    #expect(viewport.offset == CGSize(width: 80, height: -35))
    #expect(viewport.scale == 1)

    viewport.reset()
    #expect(viewport == CanvasViewportTransform())
}

@Test func viewportZoomKeepsAnchorStationary() {
    var viewport = CanvasViewportTransform(
        scale: 1,
        offset: CGSize(width: 20, height: -10)
    )
    let size = CGSize(width: 800, height: 600)
    let anchor = CGPoint(x: 200, y: 150)
    let before = contentPoint(at: anchor, viewport: viewport, size: size)

    viewport.zoom(by: 2, around: anchor, in: size)

    let after = contentPoint(at: anchor, viewport: viewport, size: size)
    #expect(abs(before.x - after.x) < 0.0001)
    #expect(abs(before.y - after.y) < 0.0001)
    #expect(viewport.scale == 2)
}

@Test func viewportClampsZoom() {
    var viewport = CanvasViewportTransform()
    let size = CGSize(width: 500, height: 500)
    let center = CGPoint(x: 250, y: 250)

    viewport.zoom(by: 100, around: center, in: size)
    #expect(viewport.scale == CanvasViewportTransform.maximumScale)

    viewport.zoom(by: 0.0001, around: center, in: size)
    #expect(viewport.scale == CanvasViewportTransform.minimumScale)
}

private func contentPoint(
    at screenPoint: CGPoint,
    viewport: CanvasViewportTransform,
    size: CGSize
) -> CGPoint {
    let center = CGPoint(x: size.width / 2, y: size.height / 2)
    return CGPoint(
        x: center.x + (screenPoint.x - center.x - viewport.offset.width) / viewport.scale,
        y: center.y + (screenPoint.y - center.y - viewport.offset.height) / viewport.scale
    )
}
