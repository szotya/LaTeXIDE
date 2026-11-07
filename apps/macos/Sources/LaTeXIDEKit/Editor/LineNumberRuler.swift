import AppKit
final class LineNumberRuler: NSRulerView {
    weak var source: SourceTextView?
    init(scrollView: NSScrollView, source: SourceTextView) {
        self.source = source
        super.init(scrollView: scrollView, orientation: .verticalRuler)
        clientView = source; ruleThickness = 56
    }
    required init(coder: NSCoder) { fatalError("init(coder:) is not used") }
    /// Marked text changes display geometry before the core commits a transaction.
    var displayedLineMap: SourceLineMap {
        guard let source else { return SourceLineMap() }
        return source.adapter?.displayedLineMap(for: source) ?? SourceLineMap(source.string)
    }
    override func drawHashMarksAndLabels(in rect: NSRect) {
        guard let view = source, let theme = view.editorTheme,
              let manager = view.layoutManager, let container = view.textContainer else { return }
        theme.ns("editor").setFill(); bounds.fill()
        let visible = view.visibleRect.offsetBy(dx: -view.textContainerOrigin.x, dy: -view.textContainerOrigin.y)
        manager.ensureLayout(forBoundingRect: visible, in: container)
        let glyphs = manager.glyphRange(forBoundingRectWithoutAdditionalLayout: visible, in: container)
        let lines = displayedLineMap
        let selected = lines.position(at: view.selectedRange().location).line
        let font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        var glyph = glyphs.location
        func draw(_ number: Int, at y: CGFloat) {
            let string = String(number) as NSString
            let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: theme.ns(number == selected ? "text" : "secondary")]
            let size = string.size(withAttributes: attrs)
            string.draw(at: NSPoint(x: ruleThickness - 16 - size.width, y: y + 1), withAttributes: attrs)
        }
        while glyphs.location != NSNotFound && glyph < NSMaxRange(glyphs) {
            var range = NSRange()
            let fragment = manager.lineFragmentRect(forGlyphAt: glyph, effectiveRange: &range)
            let character = manager.characterIndexForGlyph(at: glyph)
            let number = lines.position(at: character).line
            if lines.starts[number - 1] == character {
                let point = convert(NSPoint(x: 0, y: fragment.minY + view.textContainerOrigin.y), from: view)
                draw(number, at: point.y)
            }
            guard NSMaxRange(range) > glyph else { break }; glyph = NSMaxRange(range)
        }
        let extra = manager.extraLineFragmentRect
        let count = view.textStorage?.length ?? 0
        let endsInLineFeed = count > 0 && view.textStorage?.mutableString.character(at: count - 1) == 10
        if count == 0 || (endsInLineFeed && extra.intersects(visible)) {
            let point = convert(NSPoint(x: 0, y: extra.minY + view.textContainerOrigin.y), from: view)
            draw(lines.starts.count, at: point.y)
        }
    }
}
