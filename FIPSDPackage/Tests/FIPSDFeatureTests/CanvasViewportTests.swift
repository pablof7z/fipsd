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
    let before = viewport.contentPoint(at: anchor, in: size)

    viewport.zoom(by: 2, around: anchor, in: size)

    let after = viewport.contentPoint(at: anchor, in: size)
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

@Test func drawingTransformMatchesContentPointInverse() {
    let viewport = CanvasViewportTransform(
        scale: 3.25,
        offset: CGSize(width: 71, height: -46)
    )
    let size = CGSize(width: 920, height: 640)
    let contentPoint = CGPoint(x: 177, y: 508)
    let viewportPoint = contentPoint.applying(viewport.drawingTransform(in: size))
    let roundTrip = viewport.contentPoint(at: viewportPoint, in: size)

    #expect(abs(roundTrip.x - contentPoint.x) < 0.0001)
    #expect(abs(roundTrip.y - contentPoint.y) < 0.0001)
}
