import SwiftUI

struct CohortArtifactCanvas {
    let frame: RenderFrame
    let size: CGSize

    func draw(context: inout GraphicsContext) {
        // Reconcile both intents: draw from the engine-authoritative world
        // coordinates (render-truth `worldPoint`) but place them with the
        // fit-to-content viewport so a sparse cohort set fills the canvas
        // instead of huddling in a corner — matching CohortCanvas's approach.
        let cohorts = frame.artifactCohorts
        guard !cohorts.isEmpty else { return }
        let viewport = WorldViewport(points: cohorts.map(\.worldPoint), in: size)
        let largest = cohorts.map(\.population).max() ?? 1
        for cohort in cohorts {
            let point = viewport.project(cohort.worldPoint)
            let ratio = sqrt(Double(cohort.population) / Double(max(1, largest)))
            let diameter = max(14, min(44, 14 + CGFloat(ratio) * 30))
            let rect = CGRect(
                x: point.x - diameter / 2, y: point.y - diameter / 2,
                width: diameter, height: diameter
            )
            context.fill(
                Path(ellipseIn: rect),
                with: .color(color(cohort.region).opacity(0.78))
            )
            context.draw(
                Text(cohort.population.formatted(.number.notation(.compactName)))
                    .font(.caption2).foregroundStyle(.white),
                at: point
            )
            context.draw(
                Text("d\(cohort.depthStart.formatted(.number.notation(.compactName)))–\(cohort.depthEnd.formatted(.number.notation(.compactName)))")
                    .font(.system(size: 8)).foregroundStyle(.secondary),
                at: CGPoint(x: point.x, y: point.y + diameter / 2 + 9)
            )
        }
    }

    private func color(_ region: String) -> Color {
        let value = region.utf8.reduce(0) { ($0 &* 31) &+ Int($1) }
        return Color(hue: Double(value & 255) / 255, saturation: 0.62, brightness: 0.86)
    }
}
