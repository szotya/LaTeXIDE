import XCTest
import AppKit
@testable import LaTeXIDEKit

final class NativeColoringTests: XCTestCase {
    private final class EditNotifications: @unchecked Sendable { var count = 0 }
    @MainActor func testSyntaxRefreshDoesNotEditAttributedSourceStorage() throws {
        _ = NSApplication.shared
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let root = parent.appendingPathComponent("syntax-layout-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let source = "\\alpha " + String(repeating: "A", count: 16_384)
        try Data(source.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView), storage = try XCTUnwrap(view.textStorage)
        let before = NSAttributedString(attributedString: storage), edits = EditNotifications()
        let observer = NotificationCenter.default.addObserver(forName: NSTextStorage.didProcessEditingNotification, object: storage, queue: .main) { _ in edits.count += 1 }
        defer { NotificationCenter.default.removeObserver(observer) }
        coordinator.applyColoring(view, theme: coordinator.theme)
        XCTAssertEqual(edits.count, 0, "Syntax colors must not invalidate source glyphs through storage editing")
        XCTAssertTrue(storage.isEqual(to: before))
        XCTAssertEqual(view.layoutManager?.temporaryAttribute(.foregroundColor, atCharacterIndex: 0, effectiveRange: nil) as? NSColor, coordinator.theme.ns("command"))
        XCTAssertEqual(store.revision, 1); XCTAssertFalse(store.canUndo)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data(source.utf8))
    }
    @MainActor func testRecolorChangesOnlyPresentationAndPreservesSelection() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-color-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = "\\section{Árvíz 😀}\n% komment\n"
        try Data(source.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let coordinator = NativeEditor.Coordinator(store)
        let view = SourceTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 300))
        view.adapter = coordinator; view.delegate = coordinator; view.allowsUndo = false; view.string = source
        let selection = NSRange(location: 9, length: 5); view.setSelectedRange(selection)
        let dark = ThemeCatalog.shared.palette("graphite").resolve(dark: true)
        coordinator.applyColoring(view, theme: dark)
        XCTAssertEqual(view.string, source)
        XCTAssertEqual(view.selectedRange(), selection)
        XCTAssertEqual(store.revision, 1)
        XCTAssertFalse(store.canUndo)
        XCTAssertEqual(view.layoutManager?.temporaryAttribute(.foregroundColor, atCharacterIndex: 0, effectiveRange: nil) as? NSColor, dark.ns("command"))
        XCTAssertEqual(view.layoutManager?.temporaryAttribute(.foregroundColor, atCharacterIndex: 9, effectiveRange: nil) as? NSColor, dark.ns("text"))
        let light = ThemeCatalog.shared.palette("ocean").resolve(dark: false)
        coordinator.applyColoring(view, theme: light)
        XCTAssertEqual(view.layoutManager?.temporaryAttribute(.foregroundColor, atCharacterIndex: 0, effectiveRange: nil) as? NSColor, light.ns("command"))
        XCTAssertEqual(view.selectedRange(), selection)
        XCTAssertEqual(store.revision, 1)
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testThemeChangeDefersWhileNativeMarkedTextIsActive() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-theme-ime-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("abc".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let coordinator = NativeEditor.Coordinator(store)
        let view = SourceTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 300))
        view.adapter = coordinator; view.delegate = coordinator; view.allowsUndo = false; view.string = store.text
        view.setMarkedText("日本", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: 0, length: 0))
        coordinator.applyColoring(view, theme: ThemeCatalog.shared.palette("dune").resolve(dark: false))
        XCTAssertTrue(view.hasMarkedText()); XCTAssertTrue(store.isComposing)
        XCTAssertEqual(store.revision, 1)
        view.unmarkText()
        XCTAssertEqual(store.text, "日本abc"); XCTAssertEqual(store.revision, 2)
        store.undo(); XCTAssertEqual(store.text, "abc")
        XCTAssertTrue(store.requestTermination())
    }
}
