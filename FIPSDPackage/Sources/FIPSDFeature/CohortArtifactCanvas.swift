import SwiftUI

struct CohortArtifactCanvas {
    let state: CohortArtifactState
    let size: CGSize

    func draw(context: inout GraphicsContext) {
        let columns = 8
        let margin: CGFloat = 56
        let width = max(1, size.width - margin * 2)
        let height = max(1, size.height - margin * 2)
        let rows = max(1, Int(ceil(Double(state.cohorts.count) / Double(columns))))
        let largest = state.cohorts.map(\.population).max() ?? 1
        for (index, cohort) in state.cohorts.enumerated() {
            let column = index % columns
            let row = index / columns
            let point = CGPoint(
                x: margin + width * (CGFloat(column) + 0.5) / CGFloat(columns),
                y: margin + height * (CGFloat(row) + 0.5) / CGFloat(rows)
            )
            let ratio = sqrt(Double(cohort.population) / Double(max(1, largest)))
            let diameter = 18 + 36 * ratio
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
