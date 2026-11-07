import AppKit
import CoreText
import Darwin

/// Keep native caret geometry bounded without wrapping long fixed-pitch source.
final class SourceTypesetter: NSATSTypesetter {
    static let minimumSourceRowLength = 1024
    var isComposing = false
    private struct Metrics {
        let advance: CGFloat
        let height: CGFloat
        let baseline: CGFloat
    }
    /// Available only after actual fixed-pitch metrics were validated.
    func stableAdvance(for font: NSFont) -> CGFloat? { cachedFont == font ? cachedMetrics?.advance : nil }
    private var cachedFont: NSFont?
    private var cachedStyle: NSParagraphStyle?
    private var cachedLeading = false
    private var cachedMetrics: Metrics?
    private static let nonPrintable = CharacterSet(charactersIn: Unicode.Scalar(32)!...Unicode.Scalar(126)!).inverted
    private static let printableBytes: [CChar] = (32...126).map { CChar($0) } + [0]

    /// A single native row has no estimated vertical positions to restore.
    static func isLongASCIIRow(_ text: String) -> Bool {
        isLongASCIIRow(text as NSString)
    }
    static func isLongASCIIRow(_ source: NSString) -> Bool {
        source.length > 8192 && isPrintableASCII(source, range: NSRange(location: 0, length: source.length))
    }

    static func isPrintableASCII(_ text: String) -> Bool {
        let source = text as NSString
        return isPrintableASCII(source, range: NSRange(location: 0, length: source.length))
    }

    static func isPrintableASCII(_ text: NSString, range: NSRange) -> Bool {
        guard range.location >= 0, range.length >= 0, range.location <= text.length,
              range.length <= text.length - range.location else { return false }
        if range.location == 0 && range.length == text.length {
            // Reject ordinary multi-line/complex text before making a full ASCII copy.
            let prefix = NSRange(location: 0, length: min(8192, text.length))
            guard text.rangeOfCharacter(from: nonPrintable, options: [], range: prefix).location == NSNotFound else { return false }
            guard let bytes = text.cString(using: String.Encoding.ascii.rawValue) else { return false }
            // NUL, controls and DEL terminate the printable span before UTF-16 length.
            return printableBytes.withUnsafeBufferPointer { strspn(bytes, $0.baseAddress!) == text.length }
        }
        return text.rangeOfCharacter(from: nonPrintable, options: [], range: range).location == NSNotFound
    }

    /// Native shaping remains authoritative for unsupported text or attributes.
    override func layoutParagraph(at origin: UnsafeMutablePointer<NSPoint>) -> Int {
        let characters = paragraphCharacterRange, separator = paragraphSeparatorCharacterRange
        guard !isComposing, characters.length >= Self.minimumSourceRowLength,
              let manager = layoutManager, let storage = manager.textStorage,
              let container = manager.textContainers.first, container.layoutOrientation == .horizontal,
              !container.widthTracksTextView, container.containerSize.width == CGFloat.greatestFiniteMagnitude,
              separator.length == 0 || (separator.length == 1 && storage.mutableString.character(at: separator.location) == 10) else {
            return super.layoutParagraph(at: origin)
        }
        let content = NSRange(location: characters.location, length: characters.length - separator.length)
        guard Self.isPrintableASCII(storage.mutableString, range: content),
              let font = storage.attribute(.font, at: content.location, effectiveRange: nil) as? NSFont else {
            return super.layoutParagraph(at: origin)
        }
        let style = storage.attribute(.paragraphStyle, at: content.location, effectiveRange: nil) as? NSParagraphStyle ?? .default
        guard style.alignment == .natural || style.alignment == .left,
              style.baseWritingDirection != .rightToLeft,
              style.firstLineHeadIndent == 0, style.headIndent == 0, style.tailIndent == 0,
              style.paragraphSpacingBefore == 0, style.paragraphSpacing == 0 else {
            return super.layoutParagraph(at: origin)
        }
        var compatible = true
        storage.enumerateAttributes(in: content, options: .longestEffectiveRangeNotRequired) { attributes, _, stop in
            let runStyle = attributes[.paragraphStyle] as? NSParagraphStyle ?? .default
            let changedGeometry: [NSAttributedString.Key] = [.kern, .baselineOffset, .obliqueness, .expansion, .strokeWidth]
            if (attributes[.font] as? NSFont) != font || runStyle != style ||
                changedGeometry.contains(where: { (attributes[$0] as? NSNumber)?.doubleValue ?? 0 != 0 }) ||
                attributes[.attachment] != nil || attributes[.glyphInfo] != nil || attributes[.writingDirection] != nil ||
                (attributes[.verticalGlyphForm] as? NSNumber)?.boolValue == true {
                compatible = false; stop.pointee = true
            }
        }
        guard compatible, let metrics = metrics(font: font, style: style, leading: manager.usesFontLeading) else {
            return super.layoutParagraph(at: origin)
        }
        guard (metrics.advance * CGFloat(content.length)).isFinite else { return super.layoutParagraph(at: origin) }
        // Noncontiguous layout may leave holes outside the requested viewport.
        // Every glyph receiving a fragment must exist, including after recoloring.
        manager.ensureGlyphs(forCharacterRange: characters)
        let glyphs = manager.glyphRange(forCharacterRange: characters, actualCharacterRange: nil)
        guard glyphs.length == characters.length else {
            return super.layoutParagraph(at: origin)
        }

        beginParagraph()
        beginLine(withGlyphAt: glyphs.location)
        let padding = container.lineFragmentPadding
        var offset = 0
        while offset < glyphs.length {
            let length = min(2048, glyphs.length - offset)
            let range = NSRange(location: glyphs.location + offset, length: length)
            let leftPadding: CGFloat = offset == 0 ? padding : 0
            let x = origin.pointee.x + (offset == 0 ? 0 : padding + CGFloat(offset) * metrics.advance)
            let visibleCount = min(length, max(0, content.length - offset))
            let rect = NSRect(x: x, y: origin.pointee.y, width: max(1, CGFloat(visibleCount) * metrics.advance + leftPadding), height: metrics.height)
            manager.setTextContainer(container, forGlyphRange: range)
            manager.setLineFragmentRect(rect, forGlyphRange: range, usedRect: rect)
            manager.setLocation(NSPoint(x: leftPadding, y: metrics.baseline), forStartOfGlyphRange: range)
            offset += length
        }
        if separator.length > 0 { manager.setNotShownAttribute(true, forGlyphAt: NSMaxRange(glyphs) - 1) }
        endLine(withGlyphRange: glyphs)
        endParagraph()
        if characters.location == 0 && separator.length == 0 && content.length == storage.length {
            (manager as? SourceLayoutManager)?.certifySourceRow(font: font, style: style, length: content.length)
        }
        origin.pointee.y += metrics.height
        return NSMaxRange(glyphs)
    }

    /// Derive baseline and row height from AppKit; cache by the actual font/style.
    private func metrics(font: NSFont, style: NSParagraphStyle, leading: Bool) -> Metrics? {
        if cachedFont == font, cachedStyle == style, cachedLeading == leading { return cachedMetrics }
        cachedFont = font; cachedStyle = style.copy() as? NSParagraphStyle; cachedLeading = leading; cachedMetrics = nil
        guard font.isFixedPitch else { return nil }
        let characters: [UniChar] = Array(32...126).map { UniChar($0) }
        var glyphs = [CGGlyph](repeating: 0, count: characters.count)
        let coreFont = font as CTFont
        guard CTFontGetGlyphsForCharacters(coreFont, characters, &glyphs, characters.count) else { return nil }
        var advances = [CGSize](repeating: .zero, count: glyphs.count)
        CTFontGetAdvancesForGlyphs(coreFont, .horizontal, glyphs, &advances, glyphs.count)
        let advance = advances[0].width
        guard advance.isFinite, advance > 0, advances.allSatisfy({ abs($0.width - advance) < 0.001 }) else { return nil }

        let storage = NSTextStorage(string: "M", attributes: [.font: font, .paragraphStyle: style])
        let manager = NSLayoutManager(), container = NSTextContainer(size: NSSize(width: 100_000, height: 100_000))
        manager.usesFontLeading = leading
        storage.addLayoutManager(manager); manager.addTextContainer(container); manager.ensureLayout(for: container)
        let row = manager.lineFragmentRect(forGlyphAt: 0, effectiveRange: nil)
        let baseline = manager.location(forGlyphAt: 0).y
        guard row.height.isFinite, row.height > 0, baseline.isFinite else { return nil }
        let result = Metrics(advance: advance, height: row.height, baseline: baseline)
        cachedMetrics = result
        return result
    }
}
