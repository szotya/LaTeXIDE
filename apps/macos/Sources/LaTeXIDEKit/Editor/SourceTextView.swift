import AppKit
import Darwin
final class SourceTextView: NSTextView {
    private let sourceSurface = SourceViewportSurface()
    private var rectangleInvalidation = false
    override var needsDisplay: Bool {
        didSet { if needsDisplay && !rectangleInvalidation { sourceSurface.invalidate() } }
    }
    override var wantsUpdateLayer: Bool { true }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        sourceSurface.layout(in: self); sourceSurface.invalidate(); needsDisplay = true
    }
    override func layout() { super.layout(); sourceSurface.layout(in: self) }
    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        sourceSurface.layout(in: self); sourceSurface.invalidate(); needsDisplay = true
    }
    override func updateLayer() { sourceSurface.update(in: self) { super.draw($0) } }
    override func setNeedsDisplay(_ rect: NSRect) { setNeedsDisplay(rect, avoidAdditionalLayout: false) }
    override func setNeedsDisplay(_ rect: NSRect, avoidAdditionalLayout flag: Bool) {
        let previous = rectangleInvalidation; rectangleInvalidation = true
        defer { rectangleInvalidation = previous }
        sourceSurface.layout(in: self); sourceSurface.invalidate(rect, in: self)
        let viewport = visibleRect
        guard enclosingScrollView != nil, !viewport.isEmpty, !viewport.isNull,
              viewport.origin.x.isFinite, viewport.origin.y.isFinite,
              viewport.width.isFinite, viewport.height.isFinite else {
            super.setNeedsDisplay(rect, avoidAdditionalLayout: flag); return
        }
        let clipped = rect.intersection(viewport)
        if !clipped.isEmpty, !clipped.isNull { super.setNeedsDisplay(clipped, avoidAdditionalLayout: flag) }
    }
    weak var adapter: NativeEditor.Coordinator?
    var editorTheme: ResolvedTheme?
    /// Core ranges address literal UTF-16, including canonically equivalent text.
    func hasLiteralText(_ expected: String) -> Bool {
        guard let storage = textStorage else { return false }
        let source = storage.mutableString, other = expected as NSString, count = storage.length
        guard count == other.length else { return false }
        if count <= 4096 { return source.compare(expected, options: .literal) == .orderedSame }
        // Bulk UTF-16 reads avoid per-character CFStorage comparison on large text.
        // Scratch space stays bounded even for a complete multi-megabyte buffer.
        let capacity = 16_384
        let left = UnsafeMutablePointer<unichar>.allocate(capacity: capacity)
        let right = UnsafeMutablePointer<unichar>.allocate(capacity: capacity)
        defer { left.deallocate(); right.deallocate() }
        var offset = 0
        while offset < count {
            let length = min(capacity, count - offset), range = NSRange(location: offset, length: min(capacity, count - offset))
            source.getCharacters(left, range: range); other.getCharacters(right, range: range)
            if memcmp(left, right, length * MemoryLayout<unichar>.stride) != 0 { return false }
            offset += length
        }
        return true
    }
    private var isUnwrappedSource: Bool {
        textContainer?.widthTracksTextView == false && textContainer?.containerSize.width == CGFloat.greatestFiniteMagnitude
    }
    /// Restrict noncontiguous layout to one ASCII row; multi-line scroll stays exact.
    func prepareSourceLayout(for text: String? = nil) {
        guard let manager = layoutManager, manager.typesetter is SourceTypesetter else { return }
        let source: NSString
        if let text { source = text as NSString }
        else if let storage = textStorage { source = storage.mutableString }
        else { return }
        let enabled = isUnwrappedSource && !hasMarkedText() && ((text == nil && (manager as? SourceLayoutManager)?.hasStableSourceRow() == true) || SourceTypesetter.isLongASCIIRow(source))
        if manager.allowsNonContiguousLayout != enabled { manager.allowsNonContiguousLayout = enabled }
    }
    private func setSourceComposition(_ active: Bool) {
        guard let manager = layoutManager, let typesetter = manager.typesetter as? SourceTypesetter,
              typesetter.isComposing != active else { return }
        typesetter.isComposing = active
        if active { manager.allowsNonContiguousLayout = false }
        manager.invalidateLayout(forCharacterRange: NSRange(location: 0, length: textStorage?.length ?? 0), actualCharacterRange: nil)
    }
    private var usesSourceLineGeometry: Bool {
        isUnwrappedSource && layoutManager?.typesetter is SourceTypesetter
    }
    /// Horizontal fragments are parts of a line, not additional accessible lines.
    override func accessibilityLine(for index: Int) -> Int {
        let source: NSString = textStorage?.mutableString ?? (string as NSString)
        guard usesSourceLineGeometry, source.length > 0, index >= 0, index <= source.length else {
            return super.accessibilityLine(for: index)
        }
        var position = 0, line = 0
        while position < source.length {
            var start = 0, end = 0, contentsEnd = 0
            source.getLineStart(&start, end: &end, contentsEnd: &contentsEnd, for: NSRange(location: position, length: 0))
            if index < end || (index == end && end == contentsEnd) { return line }
            if end <= position { break }
            position = end; line += 1
        }
        return line
    }
    override func accessibilityRange(forLine line: Int) -> NSRange {
        let source: NSString = textStorage?.mutableString ?? (string as NSString)
        guard usesSourceLineGeometry, source.length > 0 else { return super.accessibilityRange(forLine: line) }
        guard line >= 0 else { return NSRange(location: 0, length: 0) }
        var position = 0, number = 0
        while true {
            var start = 0, end = 0, contentsEnd = 0
            source.getLineStart(&start, end: &end, contentsEnd: &contentsEnd, for: NSRange(location: position, length: 0))
            if number == line { return NSRange(location: position == source.length ? 0 : position, length: end - position) }
            guard end > position, end > contentsEnd else { return NSRange(location: 0, length: 0) }
            position = end; number += 1
        }
    }
    override func accessibilityInsertionPointLineNumber() -> Int {
        usesSourceLineGeometry ? accessibilityLine(for: selectedRange().location) : super.accessibilityInsertionPointLineNumber()
    }
    override func moveToBeginningOfLine(_ sender: Any?) { moveLogicalLine(toEnd: false, extending: false, sender: sender) }
    override func moveToEndOfLine(_ sender: Any?) { moveLogicalLine(toEnd: true, extending: false, sender: sender) }
    override func moveToBeginningOfLineAndModifySelection(_ sender: Any?) { moveLogicalLine(toEnd: false, extending: true, sender: sender) }
    override func moveToEndOfLineAndModifySelection(_ sender: Any?) { moveLogicalLine(toEnd: true, extending: true, sender: sender) }
    /// Unwrapped source uses hard lines, while native paragraph actions retain anchors.
    private func moveLogicalLine(toEnd: Bool, extending: Bool, sender: Any?) {
        let selected = selectedRange()
        let source: NSString = textStorage?.mutableString ?? (string as NSString)
        func bounded(at offset: Int) -> Bool {
            guard source.length > 0, let manager = layoutManager, manager.typesetter is SourceTypesetter else { return false }
            var start = 0, end = 0, contentsEnd = 0
            source.getParagraphStart(&start, end: &end, contentsEnd: &contentsEnd, for: NSRange(location: offset, length: 0))
            let content = NSRange(location: start, length: contentsEnd - start)
            guard content.length > 8192, SourceTypesetter.isPrintableASCII(source, range: content) else { return false }
            var fragment = NSRange()
            let glyph = manager.glyphIndexForCharacter(at: min(offset, source.length - 1))
            _ = manager.lineFragmentRect(forGlyphAt: glyph, effectiveRange: &fragment)
            return fragment.length <= 2048
        }
        guard !hasMarkedText(), isUnwrappedSource,
              bounded(at: selected.location) || bounded(at: NSMaxRange(selected)) else {
            if extending {
                if toEnd { super.moveToEndOfLineAndModifySelection(sender) } else { super.moveToBeginningOfLineAndModifySelection(sender) }
            } else {
                if toEnd { super.moveToEndOfLine(sender) } else { super.moveToBeginningOfLine(sender) }
            }
            return
        }
        let offset = toEnd ? NSMaxRange(selected) : selected.location
        var start = 0, end = 0, contentsEnd = 0
        source.getLineStart(&start, end: &end, contentsEnd: &contentsEnd, for: NSRange(location: offset, length: 0))
        let boundary = toEnd ? contentsEnd : start
        if selected.length == 0 && offset == boundary { scrollRangeToVisible(selected); return }
        if extending {
            if toEnd { super.moveToEndOfParagraphAndModifySelection(sender) } else { super.moveToBeginningOfParagraphAndModifySelection(sender) }
            let result = selectedRange()
            if toEnd && offset == boundary && NSMaxRange(result) > boundary {
                var nextStart = 0, nextEnd = 0, nextContentsEnd = 0
                source.getParagraphStart(&nextStart, end: &nextEnd, contentsEnd: &nextContentsEnd, for: NSRange(location: NSMaxRange(result), length: 0))
                if nextStart < nextContentsEnd { super.moveToBeginningOfParagraphAndModifySelection(sender) }
                super.moveBackwardAndModifySelection(sender)
            } else if !toEnd && offset == boundary && result.location < boundary {
                var previousStart = 0, previousEnd = 0, previousContentsEnd = 0
                source.getParagraphStart(&previousStart, end: &previousEnd, contentsEnd: &previousContentsEnd, for: NSRange(location: result.location, length: 0))
                if previousStart < previousContentsEnd { super.moveToEndOfParagraphAndModifySelection(sender) }
                super.moveForwardAndModifySelection(sender)
            }
        } else {
            if toEnd { super.moveToEndOfParagraph(sender) } else { super.moveToBeginningOfParagraph(sender) }
        }
    }
    override func mouseDown(with event: NSEvent) {
        adapter?.cancelViewportRestore()
        if followDefinition(with: event) { return }
        super.mouseDown(with: event)
    }
    func followDefinition(with event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        guard event.type == .leftMouseDown, flags == .command,
              let adapter, !adapter.rendering, !adapter.store.isComposing, !hasMarkedText(),
              adapter.document == adapter.store.selection, adapter.project == adapter.store.root,
              adapter.renderedRevision == adapter.store.revision, hasLiteralText(adapter.store.text),
              let manager = layoutManager, let container = textContainer, manager.numberOfGlyphs > 0 else { return false }
        var point = convert(event.locationInWindow, from: nil)
        point.x -= textContainerOrigin.x; point.y -= textContainerOrigin.y
        guard point.x >= 0, point.y >= 0 else { return false }
        let glyph = manager.glyphIndex(for: point, in: container)
        guard glyph < manager.numberOfGlyphs,
              manager.boundingRect(forGlyphRange: NSRange(location: glyph, length: 1), in: container).contains(point) else { return false }
        let offset = manager.characterIndexForGlyph(at: glyph)
        adapter.completion.dismiss()
        return adapter.store.goToDefinition(at: offset)
    }
    override func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        adapter?.cancelViewportRestore()
        guard adapter?.beginComposition() == true else { return }
        setSourceComposition(true)
        super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange)
        publishNativeInput()
    }
    override func unmarkText() {
        super.unmarkText(); setSourceComposition(false); adapter?.finishComposition(self)
        prepareSourceLayout()
        publishNativeInput()
    }
    override func insertText(_ string: Any, replacementRange: NSRange) {
        adapter?.cancelViewportRestore()
        let replacement = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
        if !SourceTypesetter.isPrintableASCII(replacement) { layoutManager?.allowsNonContiguousLayout = false }
        super.insertText(string, replacementRange: replacementRange)
        if !hasMarkedText() { setSourceComposition(false); adapter?.finishComposition(self) }
        publishNativeInput()
    }
    /// Native text/core acknowledgment and its visible pixels finish together.
    /// NSWindow display alone can leave wantsUpdateLayer work for a later turn.
    private func publishNativeInput() {
        guard window != nil else { return }
        layoutSubtreeIfNeeded(); updateLayer()
    }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // A search field or assistant composer owns its native text history.
        // NSView's equivalent traversal can reach this source view without focus.
        if let window, window.firstResponder !== self { return false }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if flags.contains(.command), flags.intersection([.option, .control]).isEmpty,
           event.charactersIgnoringModifiers?.lowercased() == "z" {
            adapter?.completion.dismiss()
            if hasMarkedText() { unmarkText() }
            if flags.contains(.shift) { adapter?.store.redo() } else { adapter?.store.undo() }
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result { adapter?.completion.dismiss() }
        return result
    }
    override func drawBackground(in rect: NSRect) {
        super.drawBackground(in: rect)
        guard let theme = editorTheme, selectedRange().length == 0, !hasMarkedText(),
              let manager = layoutManager, let container = textContainer else { return }
        let count = textStorage?.length ?? 0
        let endsInLineFeed = count > 0 && textStorage?.mutableString.character(at: count - 1) == 10
        var line: NSRect
        if count == 0 || (selectedRange().location == count && endsInLineFeed) { line = manager.extraLineFragmentRect }
        else {
            let glyph = manager.glyphIndexForCharacter(at: min(max(0, count - 1), selectedRange().location))
            line = manager.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        }
        if line.height == 0 { line.size.height = (font?.pointSize ?? 14) * 1.5 }
        line.origin.y += textContainerOrigin.y; line.origin.x = 0; line.size.width = bounds.width
        theme.ns("currentLine").setFill(); line.intersection(rect).fill()
        _ = container
    }
}
