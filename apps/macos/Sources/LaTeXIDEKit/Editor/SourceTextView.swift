import AppKit
final class SourceTextView: NSTextView {
    weak var adapter: NativeEditor.Coordinator?
    var editorTheme: ResolvedTheme?
    override func mouseDown(with event: NSEvent) {
        if followDefinition(with: event) { return }
        super.mouseDown(with: event)
    }
    func followDefinition(with event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        guard event.type == .leftMouseDown, flags == .command,
              let adapter, !adapter.rendering, !adapter.store.isComposing, !hasMarkedText(),
              adapter.document == adapter.store.selection, adapter.project == adapter.store.root,
              adapter.renderedRevision == adapter.store.revision, string == adapter.store.text,
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
        guard adapter?.beginComposition() == true else { return }
        super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange)
    }
    override func unmarkText() { super.unmarkText(); adapter?.finishComposition(self) }
    override func insertText(_ string: Any, replacementRange: NSRange) {
        super.insertText(string, replacementRange: replacementRange)
        if !hasMarkedText() { adapter?.finishComposition(self) }
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
        let count = (string as NSString).length
        var line: NSRect
        if count == 0 || (selectedRange().location == count && string.hasSuffix("\n")) { line = manager.extraLineFragmentRect }
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
