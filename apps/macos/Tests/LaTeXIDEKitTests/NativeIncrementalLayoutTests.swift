import AppKit
import XCTest
@testable import LaTeXIDEKit

private final class GlyphDemand: NSObject, NSLayoutManagerDelegate {
    var count = 0
    func layoutManager(_ manager: NSLayoutManager, shouldGenerateGlyphs glyphs: UnsafePointer<CGGlyph>, properties: UnsafePointer<NSLayoutManager.GlyphProperty>, characterIndexes: UnsafePointer<Int>, font: NSFont, forGlyphRange range: NSRange) -> Int {
        count += range.length
        return 0
    }
}
final class NativeIncrementalLayoutTests: XCTestCase {
    @MainActor private func layout(_ source: String? = nil) -> (NSTextStorage, SourceLayoutManager, NSLayoutManager, NSTextContainer, NSTextContainer, GlyphDemand, GlyphDemand) {
        _ = NSApplication.shared
        let font = NSFont.monospacedSystemFont(ofSize: 14, weight: .regular)
        let style = NSMutableParagraphStyle(); style.lineHeightMultiple = 1.4
        let storage = NSTextStorage(string: source ?? String(repeating: "ABC abc 012 +-*/ ", count: 1024), attributes: [.font: font, .paragraphStyle: style, .foregroundColor: NSColor.black, .underlineStyle: NSUnderlineStyle.single.rawValue])
        let optimized = SourceLayoutManager(), reference = NSLayoutManager()
        let first = NSTextContainer(size: NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude))
        let second = NSTextContainer(size: first.containerSize)
        first.widthTracksTextView = false; second.widthTracksTextView = false
        let demand = GlyphDemand(), otherDemand = GlyphDemand()
        optimized.delegate = demand; reference.delegate = otherDemand
        storage.addLayoutManager(optimized); storage.addLayoutManager(reference)
        optimized.addTextContainer(first); reference.addTextContainer(second)
        optimized.typesetter = SourceTypesetter(); reference.typesetter = SourceTypesetter()
        optimized.ensureLayout(for: first); reference.ensureLayout(for: second)
        XCTAssertGreaterThanOrEqual(demand.count, storage.length, "The native glyph counter must actually run")
        XCTAssertGreaterThanOrEqual(otherDemand.count, storage.length)
        demand.count = 0; otherDemand.count = 0
        return (storage, optimized, reference, first, second, demand, otherDemand)
    }
    @MainActor private func assertGlyphs(_ optimized: NSLayoutManager, _ reference: NSLayoutManager, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(optimized.numberOfGlyphs, reference.numberOfGlyphs, file: file, line: line)
        for index in 0..<min(optimized.numberOfGlyphs, reference.numberOfGlyphs) {
            XCTAssertEqual(optimized.cgGlyph(at: index), reference.cgGlyph(at: index), "glyph \(index)", file: file, line: line)
            XCTAssertEqual(optimized.propertyForGlyph(at: index), reference.propertyForGlyph(at: index), "property \(index)", file: file, line: line)
            XCTAssertEqual(optimized.characterIndexForGlyph(at: index), reference.characterIndexForGlyph(at: index), file: file, line: line)
        }
    }
    @MainActor func testSameLengthASCIIChangesReuseGeometryAndNotifyOtherLayoutManager() {
        let (storage, optimized, reference, first, second, demand, otherDemand) = layout()
        for (offset, replacement) in [(0, " "), (2047, "Z "), (storage.length - 2, " Q")] {
            let range = NSRange(location: offset, length: (replacement as NSString).length)
            storage.replaceCharacters(in: range, with: replacement)
            optimized.ensureLayout(for: first); reference.ensureLayout(for: second)
            XCTAssertEqual(demand.count, 0, "Uniform fixed-pitch replacement must reuse valid glyph geometry")
            XCTAssertGreaterThan(otherDemand.count, 0, "A second native layout manager still receives the edit")
            assertGlyphs(optimized, reference)
            otherDemand.count = 0
        }
    }
    @MainActor func testOneKiBUniformRowAlsoReusesNativeGlyphGeometry() {
        let (storage, optimized, reference, first, second, demand, otherDemand) = layout(String(repeating: "A", count: 1024))
        storage.replaceCharacters(in: NSRange(location: 0, length: 1), with: " ")
        optimized.ensureLayout(for: first); reference.ensureLayout(for: second)
        XCTAssertEqual(demand.count, 0)
        XCTAssertGreaterThan(otherDemand.count, 0)
        assertGlyphs(optimized, reference)
        XCTAssertEqual(optimized.boundingRect(forGlyphRange: NSRange(location: 0, length: 1024), in: first), reference.boundingRect(forGlyphRange: NSRange(location: 0, length: 1024), in: second))
    }
    @MainActor func testUnsupportedTextAndLengthChangesUseNativeShaping() {
        let (storage, optimized, reference, first, second, demand, otherDemand) = layout()
        for replacement in ["é", "🙂", "e\u{301}", "\t", "\n", "界", "", "ABC"] {
            demand.count = 0; otherDemand.count = 0
            storage.replaceCharacters(in: NSRange(location: 0, length: min(1, storage.length)), with: replacement)
            optimized.ensureLayout(for: first); reference.ensureLayout(for: second)
            XCTAssertGreaterThan(demand.count, 0)
            assertGlyphs(optimized, reference)
        }
    }
    @MainActor func testFontAndGeometryAttributeChangesInvalidateCachedRow() {
        let (storage, optimized, reference, first, second, demand, _) = layout()
        storage.addAttributes([.font: NSFont.systemFont(ofSize: 19), .kern: 1.5], range: NSRange(location: 2040, length: 40))
        optimized.ensureLayout(for: first); reference.ensureLayout(for: second)
        XCTAssertGreaterThan(demand.count, 0)
        assertGlyphs(optimized, reference)
        XCTAssertEqual(optimized.boundingRect(forGlyphRange: NSRange(location: 2040, length: 40), in: first), reference.boundingRect(forGlyphRange: NSRange(location: 2040, length: 40), in: second))
    }
}
