import AppKit
import XCTest
@testable import LaTeXIDEKit

final class SourceViewportSurfaceTests: XCTestCase {
    @MainActor private func fixture(_ source: String) throws -> (URL, WorkspaceStore, NSWindow, NSScrollView, SourceTextView) {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-source-pixels-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data(source.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        let window = NSWindow(contentRect: NSRect(x: -6000, y: -6000, width: 500, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = scroll
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        view.layoutManager?.ensureLayout(for: try XCTUnwrap(view.textContainer))
        scroll.layoutSubtreeIfNeeded(); view.layoutSubtreeIfNeeded()
        return (root, store, window, scroll, view)
    }
    @MainActor private func image(_ view: SourceTextView) throws -> (CALayer, CGImage) {
        view.layoutSubtreeIfNeeded(); view.updateLayer()
        let layer = try XCTUnwrap(view.layer?.sublayers?.first { $0.name == "LaTeX source viewport" })
        let contents = try XCTUnwrap(layer.contents)
        XCTAssertEqual(CFGetTypeID(contents as CFTypeRef), CGImage.typeID)
        return (layer, contents as! CGImage)
    }
    private func bytes(_ image: CGImage) throws -> Data { try XCTUnwrap(image.dataProvider?.data) as Data }
    @MainActor private func reference(_ view: SourceTextView, rect: NSRect, image: CGImage, composited: Bool = false) throws -> CGImage {
        let context = try XCTUnwrap(CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.bytesPerRow,
                                             space: try XCTUnwrap(image.colorSpace), bitmapInfo: image.bitmapInfo.rawValue))
        let scale = view.window?.backingScaleFactor ?? 1
        context.scaleBy(x: scale, y: scale); context.translateBy(x: -rect.minX, y: rect.maxY); context.scaleBy(x: 1, y: -1)
        context.clip(to: rect)
        NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: true)
        if composited { try XCTUnwrap(view.layer).render(in: context) } else { view.draw(rect) }
        NSGraphicsContext.restoreGraphicsState()
        return try XCTUnwrap(context.makeImage())
    }
    @MainActor private func assertPixels(_ view: SourceTextView, file: StaticString = #filePath, line: UInt = #line) throws {
        let (layer, snapshot) = try image(view)
        let expected = try reference(view, rect: layer.frame, image: snapshot)
        let actualBytes = try bytes(snapshot), expectedBytes = try bytes(expected)
        if actualBytes != expectedBytes {
            var minX = snapshot.width, minY = snapshot.height, maxX = -1, maxY = -1, count = 0
            for y in 0..<snapshot.height {
                for x in 0..<snapshot.width {
                    let offset = y * snapshot.bytesPerRow + x * 4
                    if actualBytes[offset..<offset+4] != expectedBytes[offset..<offset+4] {
                        minX = min(minX, x); maxX = max(maxX, x); minY = min(minY, y); maxY = max(maxY, y); count += 1
                    }
                }
            }
            XCTFail("Native pixel mismatch count=\(count), bounds=(\(minX),\(minY))..(\(maxX),\(maxY)), frame=\(layer.frame), selected=\(view.selectedRange())", file: file, line: line)
        }
        XCTAssertGreaterThan(Set(try bytes(snapshot)).count, 4, "Native glyph ink must be present", file: file, line: line)
        XCTAssertEqual(snapshot.width, Int((layer.bounds.width * layer.contentsScale).rounded()), file: file, line: line)
        XCTAssertEqual(snapshot.height, Int((layer.bounds.height * layer.contentsScale).rounded()), file: file, line: line)
    }
    @MainActor func testIncrementalPixelsStayExactAcrossEditsScrollSelectionAndTheme() throws {
        let (root, store, window, scroll, view) = try fixture((0..<200).map { "Sor \($0): Árvíz 😀 e\u{301}" }.joined(separator: "\n"))
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        try assertPixels(view)
        let (_, earlier) = try image(view), savedPixels = try bytes(earlier)
        view.insertText("M", replacementRange: NSRange(location: 0, length: 1))
        try assertPixels(view)
        XCTAssertEqual(try bytes(earlier), savedPixels, "Previously committed image must remain immutable")
        view.setSelectedRange(NSRange(location: 5, length: 14)); try assertPixels(view)
        scroll.contentView.scroll(to: NSPoint(x: 0, y: 850)); try assertPixels(view)
        view.backgroundColor = .white; view.textColor = .black; view.needsDisplay = true
        try assertPixels(view)
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertNil(store.failure)
    }
    @MainActor func testLongRowViewportContainsRealGlyphsAtStartFragmentBoundaryAndEnd() throws {
        let (root, store, window, scroll, view) = try fixture(String(repeating: "ABC abc 012 <> ", count: 2048))
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let manager = try XCTUnwrap(view.layoutManager), container = try XCTUnwrap(view.textContainer)
        for offset in [0, 2048, (store.text as NSString).length - 20] {
            let rect = manager.boundingRect(forGlyphRange: NSRange(location: offset, length: 1), in: container)
            scroll.contentView.scroll(to: NSPoint(x: max(0, rect.minX - 20), y: 0))
            try assertPixels(view)
            view.insertText(" ", replacementRange: NSRange(location: offset, length: 1))
            try assertPixels(view)
        }
        let (layer, _) = try image(view)
        XCTAssertLessThan(layer.bounds.height, view.bounds.height)
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertNil(store.failure)
    }
    @MainActor func testRejectedLongRowEditRestoresPixelsAndDoesNotCommitHistoryOrDisk() throws {
        let initial = String(repeating: "A", count: 16_384)
        let (root, store, window, _, view) = try fixture(initial)
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let revision = store.revision
        view.insertText("\0", replacementRange: NSRange(location: 0, length: 1))
        XCTAssertNotNil(store.failure); XCTAssertEqual(store.revision, revision)
        XCTAssertFalse(store.canUndo); XCTAssertTrue(view.hasLiteralText(initial))
        try assertPixels(view)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data(initial.utf8))
        view.insertText("B", replacementRange: NSRange(location: 0, length: 1))
        XCTAssertEqual(store.revision, revision + 1); XCTAssertTrue(store.canUndo)
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertTrue(store.text.hasPrefix("B"))
        try assertPixels(view)
    }
    @MainActor func testOneKiBInputPublishesExactNativePixelsWithBoundedRowSurface() throws {
        let (root, store, window, _, view) = try fixture(String(repeating: "A", count: 1024))
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let (layer, _) = try image(view)
        view.insertText("B", replacementRange: NSRange(location: 0, length: 1))
        let contents = try XCTUnwrap(layer.contents) as! CGImage
        let expected = try reference(view, rect: layer.frame, image: contents)
        XCTAssertEqual(try bytes(contents), try bytes(expected))
        XCTAssertLessThan(layer.bounds.height, view.bounds.height)
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertNil(store.failure)
    }
    @MainActor func testUnchangedSelectionNotificationKeepsPublishedPixels() throws {
        let (root, store, window, _, view) = try fixture("First source row\nSecond source row\nThird source row")
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        view.setSelectedRange(NSRange(location: 1, length: 0))
        let coordinator = try XCTUnwrap(view.adapter)
        coordinator.reportCaret(view)
        let (layer, _) = try image(view), before = layer.contents as AnyObject?
        coordinator.textViewDidChangeSelection(Notification(name: NSTextView.didChangeSelectionNotification, object: view))
        view.updateLayer()
        XCTAssertTrue((layer.contents as AnyObject?) === before, "An unchanged caret notification must not repaint the whole viewport")
        XCTAssertEqual(store.editorSelection, NSRange(location: 1, length: 0))
        view.setSelectedRange(NSRange(location: 19, length: 0))
        XCTAssertEqual(store.editorSelection, NSRange(location: 19, length: 0))
        try assertPixels(view)
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertNil(store.failure)
    }
    @MainActor func testIdleLayerUpdatesReusePixelsAndExplicitFullInvalidationRepaints() throws {
        let (root, _, window, _, view) = try fixture("Stable native source")
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let (layer, _) = try image(view), before = layer.contents as AnyObject?
        view.updateLayer()
        XCTAssertTrue((layer.contents as AnyObject?) === before, "An idle display callback must reuse the existing native image")
        view.needsDisplay = true; view.updateLayer()
        XCTAssertFalse((layer.contents as AnyObject?) === before, "An explicit full invalidation must still repaint")
        try assertPixels(view)
    }
    @MainActor func testNativeInputPublishesChangedViewportBeforeReturning() throws {
        let (root, store, window, _, view) = try fixture(String(repeating: "A", count: 16_384))
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let (layer, _) = try image(view), before = layer.contents as AnyObject?
        view.insertText("B", replacementRange: NSRange(location: 0, length: 1))
        XCTAssertFalse((layer.contents as AnyObject?) === before, "Native input must synchronously publish the changed viewport")
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertNil(store.failure)
    }
    @MainActor func testLayerCompositePreservesNativeOrientationAndScale() throws {
        let (root, _, window, _, view) = try fixture("Top row ABC\nsecond row 012\nthird row e\u{301} 🙂")
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let (layer, snapshot) = try image(view)
        let composite = try reference(view, rect: layer.frame, image: snapshot, composited: true)
        XCTAssertEqual(try bytes(composite), try bytes(snapshot), "The actual layer tree must preserve native glyph orientation, position and pixel scale")
    }
    @MainActor func testMarkedUnicodePaintAndCommitKeepNativeTextAndCoreAuthority() throws {
        let (root, store, window, _, view) = try fixture(String(repeating: "A", count: 16_384))
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        let original = store.text
        view.setMarkedText("日本e\u{301}", selectedRange: NSRange(location: 4, length: 0), replacementRange: NSRange(location: 1, length: 0))
        XCTAssertTrue(view.hasMarkedText()); XCTAssertTrue(store.isComposing); XCTAssertEqual(store.text, original)
        try assertPixels(view)
        view.insertText("日本e\u{301}", replacementRange: view.markedRange())
        XCTAssertFalse(view.hasMarkedText()); XCTAssertFalse(store.isComposing)
        XCTAssertTrue(view.hasLiteralText(store.text)); XCTAssertTrue(store.canUndo)
        try assertPixels(view)
        store.undo(); XCTAssertEqual(store.text, original); XCTAssertNil(store.failure)
    }
}
