import AppKit
import XCTest
@testable import LaTeXIDEKit

final class KeyboardRoutingTests: XCTestCase {
    @MainActor func testOtherLogicalKeysAndExtraModifiersDoNotUndoTheSource() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-keyboard-modifiers-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("original".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "agent "); store.save()
        let coordinator = NativeEditor.Coordinator(store), view = SourceTextView()
        view.adapter = coordinator
        for (text, modifiers) in [("y", NSEvent.ModifierFlags.command), ("z", [.command,.option]), ("z", [.command,.control])] {
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: 0,
                windowNumber: 0, context: nil, characters: text, charactersIgnoringModifiers: text, isARepeat: false, keyCode: 6)!
            XCTAssertFalse(view.performKeyEquivalent(with: event))
            XCTAssertEqual(store.text, "agent original"); XCTAssertEqual(store.revision, 2)
        }
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testUndoMenuKeepsComposerHistorySeparateFromSavedDocumentHistory() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-keyboard-menu-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("original".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "agent "); store.save()
        let window = NSWindow(contentRect: NSRect(x: -6000,y: -6000,width: 400,height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let prompt = NSTextView(frame: window.contentView!.bounds); prompt.allowsUndo = true
        window.contentView = prompt; window.makeFirstResponder(prompt)
        prompt.insertText("draft", replacementRange: NSRange(location: 0, length: 0))
        if let manager = prompt.undoManager, manager.groupingLevel > 0 { manager.endUndoGrouping() }
        store.undoCommand(in: window)
        XCTAssertEqual(prompt.string, ""); XCTAssertEqual(store.text, "agent original"); XCTAssertEqual(store.revision, 2)
        store.redoCommand(in: window)
        XCTAssertEqual(prompt.string, "draft"); XCTAssertEqual(store.text, "agent original")
        let source = SourceTextView(frame: prompt.frame)
        window.contentView = source; window.makeFirstResponder(source)
        store.undoCommand(in: window)
        XCTAssertEqual(store.text, "original"); XCTAssertEqual(store.revision, 3)
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testSourceDoesNotConsumeUndoIntendedForAssistantComposer() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-keyboard-focus-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("original".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "agent "); store.save()
        let coordinator = NativeEditor.Coordinator(store)
        let source = SourceTextView(frame: NSRect(x: 0,y: 0,width: 400,height: 200)); source.adapter = coordinator; source.string = store.text
        let prompt = NSTextView(frame: NSRect(x: 0,y: 210,width: 400,height: 100)); prompt.allowsUndo = true
        let window = NSWindow(contentRect: NSRect(x: -6000,y: -6000,width: 400,height: 320), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        window.contentView?.addSubview(source); window.contentView?.addSubview(prompt)
        XCTAssertTrue(window.makeFirstResponder(prompt))
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: 16)!
        XCTAssertFalse(source.performKeyEquivalent(with: event))
        XCTAssertEqual(store.text, "agent original"); XCTAssertEqual(store.revision, 2)
        store.save(); XCTAssertTrue(store.requestTermination())
    }
}
