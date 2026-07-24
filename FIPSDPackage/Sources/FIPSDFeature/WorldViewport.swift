import CoreGraphics
import Foundation

/// Maps stable synthetic world coordinates into a canvas by fitting the actual
/// bounding box of the rendered points, so sparse layouts fill the view instead
/// of collapsing into a corner. Scaling is uniform to keep circles round and
/// preserve relative geometry; `minSpan` caps the zoom so a handful of
/// near-coincident points don't blow up to fill the whole canvas.
struct WorldViewport: Equatable, Sendable {
    /// Canvas size used when logging renderer evidence, so a run's on-screen
    /// geometry is reviewable from the JSONL alone without knowing the live
    /// window size.
    static let referenceCanvas = CGSize(width: 1440, height: 900)

    let scale: CGFloat
    let center: RenderWorldPoint
    let size: CGSize
    let contentMin: RenderWorldPoint
    let contentMax: RenderWorldPoint

    init(
        points: [RenderWorldPoint],
        in size: CGSize,
        margin: CGFloat = 54,
        minSpan: Double = 0.12
    ) {
        self.size = size
        guard let first = points.first else {
            scale = 1
            center = RenderWorldPoint(x: 0, y: 0)
            contentMin = center
            contentMax = center
            return
        }
        var minX = first.x, maxX = first.x
        var minY = first.y, maxY = first.y
        for point in points {
            minX = min(minX, point.x); maxX = max(maxX, point.x)
            minY = min(minY, point.y); maxY = max(maxY, point.y)
        }
        contentMin = RenderWorldPoint(x: minX, y: minY)
        contentMax = RenderWorldPoint(x: maxX, y: maxY)
        center = RenderWorldPoint(x: (minX + maxX) / 2, y: (minY + maxY) / 2)
        let spanX = max(maxX - minX, minSpan)
        let spanY = max(maxY - minY, minSpan)
        let usableWidth = Double(max(1, size.width - margin * 2))
        let usableHeight = Double(max(1, size.height - margin * 2))
        scale = CGFloat(min(usableWidth / spanX, usableHeight / spanY))
    }

    func project(_ point: RenderWorldPoint) -> CGPoint {
        CGPoint(
            x: size.width / 2 + CGFloat(point.x - center.x) * scale,
            y: size.height / 2 + CGFloat(point.y - center.y) * scale
        )
    }
}

/// On-screen mark sizes, centralized so the live renderer and the evidence log
/// report identical geometry.
enum RenderMarkMetrics {
    static func nodeDiameter(nodeCount: Int) -> CGFloat {
        nodeCount > 5_000 ? 2.2 : nodeCount > 500 ? 3.2 : 6
    }

    static func cohortDiameter(nodeCount: Int) -> CGFloat {
        min(44, 8 + CGFloat(log2(Double(max(1, nodeCount)))) * 4)
    }
}
