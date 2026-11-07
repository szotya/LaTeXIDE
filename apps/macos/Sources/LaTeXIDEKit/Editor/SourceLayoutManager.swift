import AppKit
import CoreText

/// AppKit groups horizontal fragments into one draw range; clip that range too.
final class SourceLayoutManager: NSLayoutManager {
    // A certificate is issued only by complete, uniform one-row layout.
    // Unsupported text, attributes, length changes and composition use AppKit.

    private var certifiedFont: NSFont?
    private var certifiedStyle: NSParagraphStyle?
    private var certifiedLength = 0
    private static let geometryKeys: Set<NSAttributedString.Key> = [.font, .paragraphStyle, .foregroundColor, .backgroundColor, .underlineStyle, .underlineColor, .strikethroughStyle, .strikethroughColor]
    private func compatibleAttributes(_ storage: NSTextStorage, range: NSRange, font: NSFont, style: NSParagraphStyle) -> Bool {
        var compatible = true
        storage.enumerateAttributes(in: range, options: .longestEffectiveRangeNotRequired) { attrs, _, stop in
            if attrs.keys.contains(where: { !Self.geometryKeys.contains($0) }) || attrs[.font] as? NSFont != font || (attrs[.paragraphStyle] as? NSParagraphStyle ?? .default) != style {
                compatible = false; stop.pointee = true
            }
        }
        return compatible
    }
    func certifySourceRow(font: NSFont, style: NSParagraphStyle, length: Int) {
        guard let storage = textStorage, length == storage.length, length >= SourceTypesetter.minimumSourceRowLength,
              font == NSFont.monospacedSystemFont(ofSize: font.pointSize, weight: .regular),
              compatibleAttributes(storage, range: NSRange(location:0,length:length), font:font, style:style) else { return }
        certifiedFont=font; certifiedStyle=style.copy() as? NSParagraphStyle; certifiedLength=length
    }
    func hasStableSourceRow() -> Bool {
        certifiedFont != nil && textStorage?.length == certifiedLength && firstUnlaidGlyphIndex() == certifiedLength && (typesetter as? SourceTypesetter)?.isComposing == false && textContainers.first?.widthTracksTextView == false && textContainers.first?.containerSize.width == CGFloat.greatestFiniteMagnitude
    }
    override func processEditing(for storage: NSTextStorage, edited mask: NSTextStorageEditActions, range changed: NSRange, changeInLength delta: Int, invalidatedRange invalidated: NSRange) {
        if mask.contains(.editedCharacters), delta==0, changed.location>=0, changed.length>0, changed.length<=8192, changed.length<=storage.length, changed.location<=storage.length-changed.length,
           hasStableSourceRow(), let font=certifiedFont, let style=certifiedStyle,
           SourceTypesetter.isPrintableASCII(storage.mutableString,range:changed),compatibleAttributes(storage,range:changed,font:font,style:style) {
            var chars=[UniChar](repeating:0,count:changed.length),glyphs=[CGGlyph](repeating:0,count:changed.length)
            storage.mutableString.getCharacters(&chars,range:changed)
            if CTFontGetGlyphsForCharacters(font as CTFont,chars,&glyphs,chars.count) {
                for index in chars.indices {
                    replaceGlyph(at:changed.location+index,withGlyph:NSGlyph(glyphs[index]))
                    setIntAttribute(1,value:chars[index]==32 ? 1:0,forGlyphAt:changed.location+index)
                }
                removeTemporaryAttribute(.foregroundColor,forCharacterRange:changed)
                invalidateDisplay(forCharacterRange:changed)
                return
            }
        }
        certifiedFont=nil; certifiedStyle=nil; certifiedLength=0
        super.processEditing(for:storage,edited:mask,range:changed,changeInLength:delta,invalidatedRange:invalidated)
    }

    func visibleSourceGlyphs(in requested: NSRange, at origin: NSPoint, clip: NSRect) -> NSRange? {
        guard requested.length > 0, requested.location != NSNotFound,
              let typesetter = typesetter as? SourceTypesetter, !typesetter.isComposing,
              let storage = textStorage, (hasStableSourceRow() || SourceTypesetter.isLongASCIIRow(storage.mutableString)),
              let container = textContainers.first, !container.widthTracksTextView,
              container.containerSize.width == CGFloat.greatestFiniteMagnitude,
              let font = storage.attribute(.font, at: 0, effectiveRange: nil) as? NSFont,
              !clip.isInfinite, !clip.isNull else { return nil }
        var fragment = NSRange()
        let first = lineFragmentRect(forGlyphAt: 0, effectiveRange: &fragment)
        // Native fallback produces one complete fragment, so it retains native drawing.
        guard fragment.length > 0, fragment.length <= 2048 else { return nil }
        let fontBounds = font.boundingRectForFont
        let margin = max(abs(fontBounds.minX), abs(fontBounds.maxX), abs(fontBounds.minY), abs(fontBounds.maxY)) + 1
        guard margin.isFinite else { return nil }
        let expanded = clip.insetBy(dx: -margin, dy: -margin).offsetBy(dx: -origin.x, dy: -origin.y)
        guard expanded.maxY >= first.minY, expanded.minY <= first.maxY else { return NSRange(location: requested.location, length: 0) }
        let left: Int, right: Int
        if hasStableSourceRow(), let advance = typesetter.stableAdvance(for: font), advance > 0 {
            let glyphOrigin = first.minX + location(forGlyphAt: 0).x
            let limit = CGFloat(storage.length - 1)
            left = Int(min(limit, max(0, floor((expanded.minX - glyphOrigin) / advance))))
            right = Int(min(limit, max(0, ceil((expanded.maxX - glyphOrigin) / advance))))
        } else {
            left = glyphIndex(for: NSPoint(x: max(0, expanded.minX), y: first.midY), in: container)
            right = glyphIndex(for: NSPoint(x: max(0, expanded.maxX), y: first.midY), in: container)
        }
        let lower = max(requested.location, max(0, min(left, right) - 1))
        let upper = min(NSMaxRange(requested), max(left, right) + 2)
        return NSRange(location: lower, length: max(0, upper - lower))
    }
    override func drawGlyphs(forGlyphRange range: NSRange, at origin: NSPoint) {
        guard let context = NSGraphicsContext.current else {
            super.drawGlyphs(forGlyphRange: range, at: origin); return
        }
        let clip = textContainers.first?.textView.map { context.cgContext.boundingBoxOfClipPath.intersection($0.visibleRect) }
            ?? context.cgContext.boundingBoxOfClipPath
        if clip.isNull || clip.isEmpty { return }
        guard let visible = visibleSourceGlyphs(in: range, at: origin, clip: clip) else {
            super.drawGlyphs(forGlyphRange: range, at: origin); return
        }
        if visible.length > 0 { super.drawGlyphs(forGlyphRange: visible, at: origin) }
    }
}
