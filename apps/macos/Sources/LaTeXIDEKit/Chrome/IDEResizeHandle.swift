import SwiftUI
import AppKit
struct IDEResizeHandle: View {
    let axis: Axis
    let label: String
    @Binding var value: Double
    let bounds: ClosedRange<Double>
    var inverse = false
    @Environment(\.ideTheme) private var theme
    @State private var origin: Double?
    @State private var hover = false
    var body: some View {
        Rectangle().fill(theme.ui("editor"))
            .overlay {
                Rectangle().fill(theme.ui(hover || origin != nil ? "accent" : "border"))
                    .frame(width: axis == .horizontal ? 1 : nil, height: axis == .vertical ? 1 : nil)
            }
            .frame(width: axis == .horizontal ? 5 : nil, height: axis == .vertical ? 5 : nil)
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0).onChanged { event in
                if origin == nil { origin = value }
                let delta = axis == .horizontal ? event.translation.width : event.translation.height
                value = min(bounds.upperBound, max(bounds.lowerBound, origin! + delta * (inverse ? -1 : 1)))
            }.onEnded { _ in origin = nil })
            .onHover { inside in
                hover = inside
                if inside { (axis == .horizontal ? NSCursor.resizeLeftRight : NSCursor.resizeUpDown).set() } else { NSCursor.arrow.set() }
            }
            .accessibilityElement().accessibilityLabel(label)
            .accessibilityAdjustableAction { direction in
                value = min(bounds.upperBound, max(bounds.lowerBound, value + (direction == .increment ? 16 : -16)))
            }
    }
}
