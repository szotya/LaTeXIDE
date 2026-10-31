import XCTest
import AppKit
@testable import LaTeXIDEKit

final class EditorViewportTests: XCTestCase {
    @MainActor func testTabSwitchAndLastHideRestoreSelectionAndViewport() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-viewport-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = (0..<200).map { "Sor \($0) · Árvíz 😀" }.joined(separator: "\n")
        try Data(source.utf8).write(to: root.appendingPathComponent("main.tex"))
        try Data("Másik fájl".utf8).write(to: root.appendingPathComponent("other.tex"))
        let store = WorkspaceStore(); store.open(root)
        var editor = NativeEditor(store: store)
        var coordinator = editor.makeCoordinator()
        var scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        view.layoutManager?.ensureLayout(for: view.textContainer!)
        let selection = NSRange(location: 4, length: 10)
        view.setSelectedRange(selection)
        scroll.contentView.scroll(to: NSPoint(x: 0, y: 850))
        let origin = scroll.contentView.bounds.origin
        coordinator.capturePresentation(view)
        store.select("other.tex"); editor.synchronize(scroll, coordinator: coordinator)
        store.select("main.tex"); editor.synchronize(scroll, coordinator: coordinator)
        XCTAssertEqual(view.selectedRange(), selection)
        XCTAssertEqual(scroll.contentView.bounds.origin.y, origin.y, accuracy: 0.5)
        store.hideTab("other.tex"); store.hideTab("main.tex")
        NativeEditor.dismantleNSView(scroll, coordinator: coordinator)
        store.select("main.tex")
        editor = NativeEditor(store: store); coordinator = editor.makeCoordinator()
        scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let recreated = try XCTUnwrap(scroll.documentView as? SourceTextView)
        XCTAssertEqual(recreated.selectedRange(), selection)
        XCTAssertEqual(scroll.contentView.bounds.origin.y, origin.y, accuracy: 0.5)
        XCTAssertEqual(store.text, source); XCTAssertEqual(store.revision, 1); XCTAssertFalse(store.canUndo)
        XCTAssertTrue(store.requestTermination())
    }

    @MainActor func testRestoredSelectionClampsBothEndpointsAtScalarBoundaries() {
        XCTAssertEqual(EditorViewState.safeSelection(NSRange(location: 2, length: 1), in: "a😀b"), NSRange(location: 1, length: 2))
        XCTAssertEqual(EditorViewState.safeSelection(NSRange(location: 2, length: 0), in: "a😀b"), NSRange(location: 1, length: 0))
        XCTAssertEqual(EditorViewState.safeSelection(NSRange(location: 99, length: Int.max), in: "a😀b"), NSRange(location: 4, length: 0))
    }
}
