import XCTest
import AppKit
@testable import LaTeXIDEKit

final class NativeProjectTests: XCTestCase {
    @MainActor func testRejectedMarkedTextRollsBackAndAllowsEditingAgain() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-rejected-ime-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("abc".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: String(repeating: "x", count: 1024 * 1024 - 1000))
        let coordinator = NativeEditor.Coordinator(store)
        let view = SourceTextView(frame: NSRect(x: 0, y: 0, width: 500, height: 300))
        view.adapter = coordinator; view.delegate = coordinator; view.allowsUndo = false; view.string = store.text
        view.setMarkedText(String(repeating: "x", count: 2000), selectedRange: NSRange(location: 2000, length: 0), replacementRange: NSRange(location: 0, length: 0))
        view.unmarkText()
        XCTAssertFalse(store.isComposing)
        XCTAssertNotNil(store.failure)
        XCTAssertEqual(view.string.utf8.count, store.text.utf8.count)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "b")
        store.undo(); store.undo()
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testProjectSwitchPreservesUnsavedWorkspace() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-switch-\(UUID().uuidString)")
        let first = root.appendingPathComponent("first"), second = root.appendingPathComponent("second")
        for folder in [first, second] {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try Data("abc".utf8).write(to: folder.appendingPathComponent("main.tex"))
        }
        defer { try? FileManager.default.removeItem(at: root) }
        let store = WorkspaceStore(); store.open(first)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "unsaved ")
        store.open(second)
        XCTAssertEqual(store.root, first)
        XCTAssertEqual(store.text, "unsaved abc")
        XCTAssertTrue(store.dirty)
        XCTAssertNotNil(store.failure)
        store.undo()
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testNativeMarkedTextCommitsOnceAndCommandUndoUsesRustHistory() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-ime-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("abc".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let coordinator = NativeEditor.Coordinator(store)
        let view = SourceTextView(frame: NSRect(x: 0, y: 0, width: 500, height: 300))
        view.adapter = coordinator; view.delegate = coordinator; view.allowsUndo = false; view.string = store.text
        view.setMarkedText("日本", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: 0, length: 0))
        XCTAssertTrue(store.isComposing)
        XCTAssertEqual(store.revision, 1)
        view.unmarkText()
        XCTAssertFalse(store.isComposing)
        XCTAssertEqual(store.text, "日本abc")
        XCTAssertEqual(store.revision, 2)
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
            windowNumber: 0, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 6)!
        XCTAssertTrue(view.performKeyEquivalent(with: event))
        XCTAssertEqual(store.text, "abc")
        XCTAssertEqual(store.revision, 3)
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testNativeProjectOpenDecodesFileEntriesAndReadsSource() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-swift-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("Árvíz 😀\n".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore()
        store.open(root)
        XCTAssertNil(store.failure)
        XCTAssertEqual(store.files, ["main.tex"])
        XCTAssertEqual(store.text, "Árvíz 😀\n")
        XCTAssertEqual(store.revision, 1)
        XCTAssertTrue(store.requestTermination())
    }
}
