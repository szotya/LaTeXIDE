import XCTest
import AppKit
@testable import LaTeXIDEKit

final class SourceTypesetterTests: XCTestCase {
    func testPrintableASCIIGuardRejectsControlsNULAndComplexText() {
        let printable = String(UnicodeScalar(32)!) + String(repeating: "A", count: 9000) + String(UnicodeScalar(126)!)
        XCTAssertTrue(SourceTypesetter.isPrintableASCII(printable))
        XCTAssertTrue(SourceTypesetter.isLongASCIIRow(printable))
        XCTAssertTrue(SourceTypesetter.isPrintableASCII(""))
        for suffix in ["\0", "\u{1f}", "\u{7f}", "\t", "\r", "\n", "é", "e\u{301}", "😀"] {
            XCTAssertFalse(SourceTypesetter.isPrintableASCII(printable + suffix))
            XCTAssertFalse(SourceTypesetter.isLongASCIIRow(printable + suffix))
        }
        let mixed = "é" + printable + "😀" as NSString
        XCTAssertTrue(SourceTypesetter.isPrintableASCII(mixed, range: NSRange(location: 1, length: (printable as NSString).length)))
        XCTAssertFalse(SourceTypesetter.isPrintableASCII(mixed, range: NSRange(location: NSNotFound, length: 1)))
    }
    @MainActor func testLiteralBufferGuardChecksCompleteUTF16AcrossBulkReadBoundaries() {
        _ = NSApplication.shared
        let view = SourceTextView(frame: .zero), prefix = String(repeating: "A", count: 16_383)
        for (source, expected, equal) in [
            ("", "", true), ("é", "e\u{301}", false), ("Å", "\u{212B}", false),
            (prefix + "😀é\0z", prefix + "😀é\0z", true),
            (prefix + "😀é\0z", prefix + "😃é\0z", false),
            (prefix + "Å" + prefix, prefix + "\u{212B}" + prefix, false),
            (prefix + prefix + "a", prefix + prefix + "b", false)
        ] {
            view.string = source
            XCTAssertEqual(view.hasLiteralText(expected), equal)
        }
        view.string = prefix + "abc"
        view.textStorage?.replaceCharacters(in: NSRange(location: 16_384, length: 1), with: "X")
        XCTAssertFalse(view.hasLiteralText(prefix + "abc"))
        XCTAssertTrue(view.hasLiteralText(prefix + "aXc"))
    }
    @MainActor private struct Fixture {
        let root: URL
        let store: WorkspaceStore
        let editor: NativeEditor
        let coordinator: NativeEditor.Coordinator
        let scroll: NSScrollView
        let view: SourceTextView
    }
    @MainActor private func fixture(_ source: String) throws -> Fixture {
        _ = NSApplication.shared
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let root = parent.appendingPathComponent("native-fragments-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data(source.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore()
        addTeardownBlock {
            try await MainActor.run {
                store.save()
                XCTAssertTrue(store.requestTermination())
                try FileManager.default.removeItem(at: root)
            }
        }
        store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        view.layoutManager?.ensureLayout(for: try XCTUnwrap(view.textContainer))
        return Fixture(root: root, store: store, editor: editor, coordinator: coordinator, scroll: scroll, view: view)
    }
    @MainActor private func extensionSequence(_ view: NSTextView, caret: Int) -> [NSRange] {
        view.setSelectedRange(NSRange(location: caret, length: 0))
        var ranges: [NSRange] = []
        view.moveToBeginningOfLineAndModifySelection(nil); ranges.append(view.selectedRange())
        view.moveToBeginningOfLineAndModifySelection(nil); ranges.append(view.selectedRange())
        view.moveToEndOfLineAndModifySelection(nil); ranges.append(view.selectedRange())
        view.moveToEndOfLineAndModifySelection(nil); ranges.append(view.selectedRange())
        return ranges
    }
    @MainActor private func stockExtensionSequence(_ source: String, caret: Int, window: NSWindow) -> [NSRange] {
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 500, height: 240))
        let stock = NSTextView(frame: scroll.bounds)
        stock.isRichText = false; stock.font = .monospacedSystemFont(ofSize: 14, weight: .regular)
        stock.isHorizontallyResizable = true
        stock.textContainer?.widthTracksTextView = false
        stock.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        stock.string = source; scroll.documentView = stock; window.contentView = scroll
        _ = window.makeFirstResponder(stock)
        return extensionSequence(stock, caret: caret)
    }
    // An unbounded native caret fragment makes a front edit visit the full MiB.
    @MainActor func testHorizontalDrawingVisitsOnlyVisibleSourceGlyphs() throws {
        let f = try fixture(String(repeating: "A", count: 16_384))
        let manager = try XCTUnwrap(f.view.layoutManager as? SourceLayoutManager), origin = f.view.textContainerOrigin
        let point = manager.boundingRect(forGlyphRange: NSRange(location: 5000, length: 50), in: try XCTUnwrap(f.view.textContainer))
        let clip = point.offsetBy(dx: origin.x, dy: origin.y)
        let visible = try XCTUnwrap(manager.visibleSourceGlyphs(in: NSRange(location: 0, length: 16_384), at: origin, clip: clip))
        XCTAssertLessThan(visible.length, 512)
        XCTAssertLessThanOrEqual(visible.location, 5000)
        XCTAssertGreaterThanOrEqual(NSMaxRange(visible), 5050)
        XCTAssertNil(manager.visibleSourceGlyphs(in: NSRange(location: 0, length: 16_384), at: origin, clip: .infinite))
        let complex = try fixture(String(repeating: "A", count: 9000) + "😀")
        let fallback = try XCTUnwrap(complex.view.layoutManager as? SourceLayoutManager)
        XCTAssertNil(fallback.visibleSourceGlyphs(in: NSRange(location: 0, length: fallback.numberOfGlyphs), at: origin, clip: clip))
    }
    @MainActor func testLargeASCIICaretGeometryStaysBoundedOnOneCompleteRow() throws {
        _ = NSApplication.shared
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let root = parent.appendingPathComponent("native-long-row-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let original = String(repeating: "A", count: 1024 * 1024)
        try Data(original.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore()
        addTeardownBlock {
            try await MainActor.run {
                store.save()
                XCTAssertTrue(store.requestTermination())
                try FileManager.default.removeItem(at: root)
            }
        }
        store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        let manager = try XCTUnwrap(view.layoutManager), container = try XCTUnwrap(view.textContainer)
        manager.ensureLayout(for: container)
        var previousX: CGFloat = -1, rowY: CGFloat?
        for offset in [0, 2047, 2048, 1024 * 1024 - 1] {
            let glyph = manager.glyphIndexForCharacter(at: offset)
            var range = NSRange()
            let fragment = manager.lineFragmentRect(forGlyphAt: glyph, effectiveRange: &range)
            XCTAssertLessThanOrEqual(range.length, 2048, "Caret geometry must not traverse the whole long paragraph")
            if let rowY { XCTAssertEqual(fragment.minY, rowY, accuracy: 0.5) } else { rowY = fragment.minY }
            let x = fragment.minX + manager.location(forGlyphAt: glyph).x
            XCTAssertGreaterThan(x, previousX)
            if offset == 2048 { XCTAssertLessThan(x - previousX, 20, "Fragment boundary must not add a gap or wrap") }
            previousX = x
        }
        XCTAssertGreaterThan(previousX, 1_000_000)
        XCTAssertGreaterThanOrEqual(manager.usedRect(for: container).maxX, previousX)
        XCTAssertEqual((view.string as NSString).compare(original, options: .literal), .orderedSame)
        XCTAssertEqual(store.text, original); XCTAssertFalse(store.dirty); XCTAssertFalse(store.canUndo)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data(original.utf8))
    }

    @MainActor func testLogicalLineNavigationAndSelectionCrossFragmentBoundaries() throws {
        let source = String(repeating: "A", count: 16_384)
        let f = try fixture(source), view = f.view
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 240), styleMask: .titled, backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = f.scroll
        defer { window.close() }
        XCTAssertTrue(window.makeFirstResponder(view))
        view.setSelectedRange(NSRange(location: 2048, length: 0))
        view.moveLeft(nil); XCTAssertEqual(view.selectedRange(), NSRange(location: 2047, length: 0))
        view.moveRight(nil); XCTAssertEqual(view.selectedRange(), NSRange(location: 2048, length: 0))
        view.setSelectedRange(NSRange(location: 5000, length: 0))
        view.moveToBeginningOfLine(nil); XCTAssertEqual(view.selectedRange(), NSRange(location: 0, length: 0))
        view.moveToEndOfLine(nil); XCTAssertEqual(view.selectedRange(), NSRange(location: 16_384, length: 0))
        view.setSelectedRange(NSRange(location: 5000, length: 0))
        view.moveToBeginningOfLineAndModifySelection(nil); XCTAssertEqual(view.selectedRange(), NSRange(location: 0, length: 5000))
        view.setSelectedRange(NSRange(location: 5000, length: 0))
        view.moveToEndOfLineAndModifySelection(nil); XCTAssertEqual(view.selectedRange(), NSRange(location: 5000, length: 11_384))
        XCTAssertEqual(f.store.text, source); XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testMarkedASCIIRetainsNativeShapingUntilOneCommit() throws {
        let source = String(repeating: "A", count: 16_384)
        let f = try fixture(source), view = f.view
        view.setMarkedText("xy", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: 5000, length: 2))
        XCTAssertTrue(view.hasMarkedText()); XCTAssertTrue(f.store.isComposing)
        XCTAssertEqual(f.store.text, source); XCTAssertFalse(f.store.canUndo)
        let manager = try XCTUnwrap(view.layoutManager), container = try XCTUnwrap(view.textContainer)
        manager.ensureLayout(for: container)
        var range = NSRange()
        _ = manager.lineFragmentRect(forGlyphAt: 5000, effectiveRange: &range)
        XCTAssertGreaterThan(range.length, 2048, "Marked text must use the native paragraph shaper")
        view.unmarkText()
        XCTAssertFalse(f.store.isComposing)
        XCTAssertEqual((f.store.text as NSString).substring(with: NSRange(location: 5000, length: 2)), "xy")
        f.store.undo(); f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        XCTAssertEqual(f.store.text, source); XCTAssertFalse(f.store.canUndo)
        XCTAssertEqual(try Data(contentsOf: f.root.appendingPathComponent("main.tex")), Data(source.utf8))
    }

    @MainActor func testLongMiddleParagraphKeepsNativeRowsAndNewlineGeometry() throws {
        let source = "prefix\n" + String(repeating: "A", count: 16_384) + "\nend"
        let f = try fixture(source), manager = try XCTUnwrap(f.view.layoutManager)
        let first = manager.lineFragmentRect(forGlyphAt: 0, effectiveRange: nil)
        var range = NSRange()
        let middle = manager.lineFragmentRect(forGlyphAt: 7 + 2048, effectiveRange: &range)
        let last = manager.lineFragmentRect(forGlyphAt: 7 + 16_384 + 1, effectiveRange: nil)
        XCTAssertLessThanOrEqual(range.length, 2048)
        XCTAssertGreaterThan(middle.minY, first.minY); XCTAssertGreaterThan(last.minY, middle.minY)
        XCTAssertEqual(f.coordinator.lineMap.position(at: 7 + 2048).line, 2)
        XCTAssertEqual(f.view.string, source)
    }

    @MainActor func testFontAndParagraphChangesMatchNativeBaselineAndHeight() throws {
        let f = try fixture(String(repeating: "A", count: 16_384))
        let font = NSFont.monospacedSystemFont(ofSize: 22, weight: .regular)
        let paragraph = NSMutableParagraphStyle(); paragraph.lineHeightMultiple = 1.8
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .paragraphStyle: paragraph]
        let storage = try XCTUnwrap(f.view.textStorage), manager = try XCTUnwrap(f.view.layoutManager)
        storage.addAttributes(attributes, range: NSRange(location: 0, length: storage.length))
        manager.ensureLayout(for: try XCTUnwrap(f.view.textContainer))
        let reference = NSTextView(frame: NSRect(x: 0, y: 0, width: 500, height: 200))
        reference.textStorage?.setAttributedString(NSAttributedString(string: "AA", attributes: attributes))
        let native = try XCTUnwrap(reference.layoutManager)
        native.ensureLayout(for: try XCTUnwrap(reference.textContainer))
        let row = manager.lineFragmentRect(forGlyphAt: 0, effectiveRange: nil)
        let expected = native.lineFragmentRect(forGlyphAt: 0, effectiveRange: nil)
        XCTAssertEqual(row.height, expected.height, accuracy: 0.01)
        XCTAssertEqual(manager.location(forGlyphAt: 0).y, native.location(forGlyphAt: 0).y, accuracy: 0.01)
        var range = NSRange(); _ = manager.lineFragmentRect(forGlyphAt: 5000, effectiveRange: &range)
        XCTAssertLessThanOrEqual(range.length, 2048)
    }

    @MainActor func testComplexTextAndTabsKeepNativeParagraphShaping() throws {
        for suffix in ["😀", "e\u{301}", "אבג", "\t"] {
            let source = String(repeating: "A", count: 9000) + suffix
            let f = try fixture(source), manager = try XCTUnwrap(f.view.layoutManager)
            var range = NSRange(); _ = manager.lineFragmentRect(forGlyphAt: 0, effectiveRange: &range)
            XCTAssertGreaterThan(range.length, 2048)
            XCTAssertEqual((f.view.string as NSString).compare(source, options: .literal), .orderedSame)
        }
    }

    @MainActor func testFullNativeSelectionCopiesAllTextToPrivatePasteboard() throws {
        let source = String(repeating: "abc XYZ 012 ", count: 1000)
        let f = try fixture(source), pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 240), styleMask: .titled, backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = f.scroll
        defer { window.close() }
        XCTAssertTrue(window.makeFirstResponder(f.view))
        f.view.selectAll(nil)
        XCTAssertEqual(f.view.selectedRange(), NSRange(location: 0, length: 12_000))
        let type = try XCTUnwrap(f.view.writablePasteboardTypes.first { $0 == .string || $0.rawValue == "NSStringPboardType" })
        XCTAssertTrue(f.view.writeSelection(to: pasteboard, types: [type]))
        XCTAssertTrue(pasteboard.string(forType: type) == source, "Private plain-text copy must retain all 12000 source bytes")
    }

    @MainActor func testVisibleLongRowGlyphsRemainPresentAfterColoring() throws {
        let f = try fixture(String(repeating: "A", count: 16_384))
        let manager = try XCTUnwrap(f.view.layoutManager)
        for index in 0..<256 {
            let glyph = manager.cgGlyph(at: index)
            XCTAssertNotEqual(glyph, 0, "Visible source glyph missing at \(index)")
            if glyph == 0 { return }
        }
    }

    @MainActor func testRepeatedLogicalLineExtensionMatchesNativeSelectionAtNewlines() throws {
        let f = try fixture("first\nsecond\nthird"), view = f.view
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 240), styleMask: .titled, backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = f.scroll
        defer { window.close() }
        let reference = stockExtensionSequence("first\nsecond\nthird", caret: 9, window: window)
        window.contentView = f.scroll
        XCTAssertTrue(window.makeFirstResponder(view))
        XCTAssertEqual(extensionSequence(view, caret: 9), reference)
        view.setSelectedRange(NSRange(location: 6, length: 0))
        view.moveToBeginningOfLine(nil); XCTAssertEqual(view.selectedRange().location, 6)
        view.setSelectedRange(NSRange(location: 12, length: 0))
        view.moveToEndOfLine(nil); XCTAssertEqual(view.selectedRange().location, 12)
    }

    @MainActor func testRepeatedExtensionOfLongMiddleRowMatchesNativeSelection() throws {
        for prefix in ["prev\n", "\n"] {
            let f = try fixture(prefix + String(repeating: "A", count: 16_384) + "\nnext"), view = f.view
            let caret = prefix.utf16.count + 5000
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 240), styleMask: .titled, backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; window.contentView = f.scroll
            defer { window.close() }
            let reference = stockExtensionSequence(prefix + String(repeating: "A", count: 16_384) + "\nnext", caret: caret, window: window)
            window.contentView = f.scroll
            XCTAssertTrue(window.makeFirstResponder(view))
            XCTAssertEqual(extensionSequence(view, caret: caret), reference)
        }
    }

    @MainActor func testUnsupportedUnicodeLineBreakKeepsNativeLineNavigation() throws {
        let f = try fixture("a\u{2028}b"), view = f.view
        view.setSelectedRange(NSRange(location: 3, length: 0))
        view.moveToBeginningOfLine(nil)
        XCTAssertEqual(view.selectedRange(), NSRange(location: 2, length: 0))
    }

    @MainActor func testLocalAccessibilityKeepsTheCompleteLongRow() throws {
        let source = String(repeating: "A", count: 16_384)
        let f = try fixture(source)
        XCTAssertEqual(f.view.accessibilityLine(for: 5000), 0)
        XCTAssertEqual(f.view.accessibilityRange(forLine: 0), NSRange(location: 0, length: 16_384))
        XCTAssertTrue(f.view.accessibilityString(for: NSRange(location: 0, length: 16_384)) == source)
        f.view.setSelectedRange(NSRange(location: 5000, length: 0))
        XCTAssertEqual(f.view.accessibilityInsertionPointLineNumber(), 0)
    }

    @MainActor func testAccessibleHardLinesMatchNativeNewlinesAndUnicodeSeparators() throws {
        for suffix in ["", "\n", "\n\nz", "\u{2028}😀\n"] {
            let source = String(repeating: "A", count: 9000) + suffix
            let f = try fixture(source)
            let stock = NSTextView(frame: NSRect(x: 0, y: 0, width: 500, height: 240))
            stock.isRichText = false; stock.font = .monospacedSystemFont(ofSize: 14, weight: .regular)
            stock.isHorizontallyResizable = true; stock.textContainer?.widthTracksTextView = false
            stock.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
            stock.string = source
            stock.layoutManager?.ensureLayout(for: try XCTUnwrap(stock.textContainer))
            for index in [0, 8999, 9000, (source as NSString).length] {
                XCTAssertEqual(f.view.accessibilityLine(for: index), stock.accessibilityLine(for: index), "Suffix \(suffix.debugDescription), index \(index)")
            }
            for line in 0..<4 {
                XCTAssertEqual(f.view.accessibilityRange(forLine: line), stock.accessibilityRange(forLine: line), "Suffix \(suffix.debugDescription), line \(line)")
            }
        }
    }

    @MainActor func testLongSingleRowRestoresHorizontalViewportAfterLastTabRecreation() throws {
        let f = try fixture(String(repeating: "A", count: 16_384))
        f.view.sizeToFit()
        let selection = NSRange(location: 7000, length: 5)
        f.view.setSelectedRange(selection)
        f.scroll.contentView.scroll(to: NSPoint(x: 5000, y: 0))
        let origin = f.scroll.contentView.bounds.origin
        XCTAssertGreaterThan(origin.x, 4000)
        f.coordinator.capturePresentation(f.view)
        f.store.hideTab("main.tex")
        NativeEditor.dismantleNSView(f.scroll, coordinator: f.coordinator)
        f.store.select("main.tex")
        let editor = NativeEditor(store: f.store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        XCTAssertEqual(view.selectedRange(), selection)
        XCTAssertEqual(scroll.contentView.bounds.origin.x, origin.x, accuracy: 0.5)
        XCTAssertEqual(scroll.contentView.bounds.origin.y, 0, accuracy: 0.5)
        XCTAssertTrue(try XCTUnwrap(view.layoutManager).allowsNonContiguousLayout)
        XCTAssertEqual(f.store.text, String(repeating: "A", count: 16_384))
    }

    @MainActor func testUnacknowledgedCanonicalEquivalentBufferIsNotColored() throws {
        let f = try fixture("é\\alpha")
        f.coordinator.rendering = true
        f.view.string = "e\u{301}\\alpha"
        f.view.textStorage?.addAttribute(.foregroundColor, value: NSColor.magenta, range: NSRange(location: 0, length: (f.view.string as NSString).length))
        f.coordinator.rendering = false
        f.coordinator.applyColoring(f.view, theme: f.coordinator.theme)
        XCTAssertEqual(f.view.textStorage?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor, .magenta)
        XCTAssertEqual(f.store.text, "é\\alpha"); XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testCanonicalEquivalentExternalEditRefreshesLiteralUTF16BeforeNextInput() throws {
        let f = try fixture("éz")
        try f.store.edit(range: NSRange(location: 0, length: 1), replacement: "e\u{301}")
        f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        let aligned = (f.view.string as NSString).compare(f.store.text, options: .literal) == .orderedSame
        XCTAssertTrue(aligned, "Canonical equality must not retain different UTF-16 offsets")
        guard aligned else { return }
        f.view.insertText("X", replacementRange: NSRange(location: 2, length: 1))
        XCTAssertNil(f.store.failure)
        XCTAssertEqual((f.view.string as NSString).compare("e\u{301}X", options: .literal), .orderedSame)
        f.store.undo(); f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        XCTAssertEqual((f.view.string as NSString).compare("e\u{301}z", options: .literal), .orderedSame)
        f.store.undo(); f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        XCTAssertEqual((f.view.string as NSString).compare("éz", options: .literal), .orderedSame)
        XCTAssertEqual(try Data(contentsOf: f.root.appendingPathComponent("main.tex")), Data("éz".utf8))
    }

    @MainActor func testCanonicalEquivalentUnacknowledgedBufferDoesNotOfferCompletion() throws {
        let f = try fixture("é \\alp")
        f.coordinator.rendering = true
        f.view.string = "e\u{301} \\alp"
        f.view.setSelectedRange(NSRange(location: 5, length: 0))
        f.coordinator.rendering = false
        f.coordinator.completion.refresh(f.view, explicit: true)
        XCTAssertNil(f.coordinator.completion.proposal)
        f.coordinator.completion.dismiss()
        XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testCanonicalEquivalentUnacknowledgedBufferCannotAcceptCompletion() throws {
        let f = try fixture("é \\alp")
        f.view.setSelectedRange(NSRange(location: 5, length: 0))
        f.coordinator.completion.refresh(f.view, explicit: true)
        XCTAssertNotNil(f.coordinator.completion.proposal)
        f.coordinator.rendering = true
        f.view.string = "e\u{301} \\alp"
        f.view.setSelectedRange(NSRange(location: 5, length: 0))
        f.coordinator.rendering = false
        XCTAssertFalse(f.coordinator.completion.accept(f.view))
        XCTAssertEqual(f.store.revision, 1)
        XCTAssertEqual(f.store.text, "é \\alp"); XCTAssertFalse(f.store.canUndo)
    }
}
