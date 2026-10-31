import XCTest
import AppKit
@testable import LaTeXIDEKit

final class CompletionTests: XCTestCase {
    @MainActor private func fixture() throws -> (URL, WorkspaceStore, NativeEditor.Coordinator, SourceTextView) {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-completion-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data("\\sec".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store); let coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0,y: 0,width: 700,height: 300)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = scroll.documentView as! SourceTextView
        view.setSelectedRange(NSRange(location: 4, length: 0))
        return (root, store, coordinator, view)
    }
    @MainActor func testAcceptedCommandIsOneUndoTransactionAndSelectsFirstArgument() throws {
        let (root, store, coordinator, view) = try fixture()
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        XCTAssertEqual(coordinator.completion.proposal?.items.first?.label, "\\section")
        XCTAssertTrue(coordinator.completion.accept(view))
        XCTAssertEqual(store.text, "\\section{}")
        XCTAssertEqual(store.revision, 2)
        XCTAssertEqual(view.selectedRange(), NSRange(location: 9, length: 0))
        store.undo(); XCTAssertEqual(store.text, "\\sec"); XCTAssertFalse(store.canUndo)
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testStaleCompletionDoesNotMutateTheNewRevision() throws {
        let (root, store, coordinator, view) = try fixture()
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "X")
        XCTAssertFalse(coordinator.completion.accept(view))
        XCTAssertEqual(store.text, "X\\sec"); XCTAssertEqual(store.revision, 2)
        store.undo(); XCTAssertEqual(store.text, "\\sec"); XCTAssertFalse(store.canUndo)
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testStaleRevisionDoesNotConsumeReturn() throws {
        let (root, store, coordinator, view) = try fixture()
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        // The authoritative revision can change before this view's next render/refresh.
        try store.edit(range: NSRange(location: 4, length: 0), replacement: "t")
        XCTAssertNotNil(coordinator.completion.proposal)
        let handled = coordinator.completion.handle(#selector(NSResponder.insertNewline(_:)), view: view)
        XCTAssertFalse(handled)
        XCTAssertNil(coordinator.completion.proposal)
        XCTAssertEqual(store.text, "\\sect")
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testStaleProposalLetsTabEditAfterCaretMoves() throws {
        let (root, store, coordinator, view) = try fixture()
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        view.setSelectedRange(NSRange(location: 0, length: 0))
        let handled = coordinator.completion.handle(#selector(NSResponder.insertTab(_:)), view: view)
        XCTAssertFalse(handled)
        if !handled { view.insertTab(nil) }
        XCTAssertEqual(store.text, "\t\\sec")
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testPopupReleasesItsControllerWhenEditorIsRemoved() throws {
        weak var reference: CompletionController?
        try autoreleasepool {
            let (root, store, coordinator, view) = try fixture()
            defer { try? FileManager.default.removeItem(at: root) }
            let window = NSWindow(contentRect: NSRect(x: -6000,y: -6000,width: 500,height: 250), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            let scroll = try XCTUnwrap(view.enclosingScrollView)
            window.contentView = scroll; window.makeFirstResponder(view)
            coordinator.completion.refresh(view)
            reference = coordinator.completion
            XCTAssertNotNil(window.childWindows?.first)
            NativeEditor.dismantleNSView(scroll, coordinator: coordinator)
            window.contentView = nil; window.close()
            XCTAssertTrue(store.requestTermination())
        }
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        XCTAssertNil(reference)
    }
    @MainActor func testMarkedTextAndDisabledPreferenceSuppressCompletion() throws {
        let (root, store, coordinator, view) = try fixture()
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completionEnabled = false; coordinator.completion.refresh(view)
        XCTAssertNil(coordinator.completion.proposal)
        coordinator.completionEnabled = true
        view.setMarkedText("日", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 4, length: 0))
        coordinator.completion.refresh(view); XCTAssertNil(coordinator.completion.proposal)
        view.unmarkText(); store.undo()
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testCommandZUsesLogicalCharacterForHungarianAndUSLayouts() throws {
        let (root, store, _, view) = try fixture()
        // The view's weak adapter needs the real coordinator retained for this test.
        let coordinator = NativeEditor.Coordinator(store); view.adapter = coordinator
        defer { try? FileManager.default.removeItem(at: root) }
        for keyCode: UInt16 in [16, 6] {
            try store.edit(range: NSRange(location: 0, length: 0), replacement: "agent ")
            store.save()
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
                windowNumber: 0, context: nil, characters: "z", charactersIgnoringModifiers: "z", isARepeat: false, keyCode: keyCode)!
            XCTAssertTrue(view.performKeyEquivalent(with: event))
            XCTAssertEqual(store.text, "\\sec")
            let redo = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command,.shift], timestamp: 0,
                windowNumber: 0, context: nil, characters: "Z", charactersIgnoringModifiers: "Z", isARepeat: false, keyCode: keyCode)!
            XCTAssertTrue(view.performKeyEquivalent(with: redo)); XCTAssertEqual(store.text, "agent \\sec")
            store.undo(); store.save()
        }
        XCTAssertTrue(store.requestTermination())
    }
}
