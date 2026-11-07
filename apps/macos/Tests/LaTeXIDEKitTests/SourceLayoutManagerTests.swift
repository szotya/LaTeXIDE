import AppKit
import XCTest
@testable import LaTeXIDEKit

final class SourceLayoutManagerTests: XCTestCase {
    @MainActor func testClippedGlyphPixelsMatchCompleteNativeDrawingAcrossHorizontalPositions() throws {
        _ = NSApplication.shared
        let source = String(repeating: "ABC abc 012 <> [] () +-*/ \\ ", count: 800)
        let font = NSFont.monospacedSystemFont(ofSize: 14, weight: .regular)
        let style = NSMutableParagraphStyle(); style.lineHeightMultiple = 1.4
        func layout(_ manager: NSLayoutManager) -> (NSTextStorage, NSTextContainer) {
            let storage = NSTextStorage(string: source, attributes: [.font: font, .foregroundColor: NSColor.black, .paragraphStyle: style])
            let container = NSTextContainer(size: NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude))
            container.widthTracksTextView = false
            storage.addLayoutManager(manager); manager.addTextContainer(container)
            manager.typesetter = SourceTypesetter(); manager.ensureLayout(for: container)
            manager.addTemporaryAttribute(.foregroundColor, value: NSColor.blue, forCharacterRange: NSRange(location: 2030, length: 80))
            return (storage, container)
        }
        let optimized = SourceLayoutManager(), reference = NSLayoutManager()
        let (storage, container) = layout(optimized), (referenceStorage, _) = layout(reference)
        let range = NSRange(location: 0, length: optimized.numberOfGlyphs)
        func pixels(_ manager: NSLayoutManager, x: CGFloat) throws -> Data {
            let image = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 400, pixelsHigh: 80, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
            let context = try XCTUnwrap(NSGraphicsContext(bitmapImageRep: image))
            NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = context
            defer { NSGraphicsContext.restoreGraphicsState() }
            NSColor.white.setFill(); NSRect(x: 0, y: 0, width: 400, height: 80).fill()
            context.cgContext.translateBy(x: -x, y: 0)
            manager.drawGlyphs(forGlyphRange: range, at: .zero)
            context.flushGraphics()
            return Data(bytes: try XCTUnwrap(image.bitmapData), count: image.bytesPerRow * image.pixelsHigh)
        }
        for glyph in [0, 2048, range.length - 20] {
            let bounds = optimized.boundingRect(forGlyphRange: NSRange(location: glyph, length: 1), in: container)
            let x = max(0, bounds.minX - 30)
            let actual = try pixels(optimized, x: x), expected = try pixels(reference, x: x)
            XCTAssertEqual(actual, expected, "Clipping must preserve native glyph pixels near \(glyph)")
            XCTAssertTrue(expected.contains { $0 != 255 }, "The reference viewport must contain real glyph ink")
        }
        XCTAssertEqual(storage.string, source); XCTAssertEqual(referenceStorage.string, source)
    }
}
