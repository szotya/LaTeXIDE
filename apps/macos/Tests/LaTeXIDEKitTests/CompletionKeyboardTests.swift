import AppKit
import XCTest
@testable import LaTeXIDEKit

final class CompletionKeyboardTests: XCTestCase {
    @MainActor private func checkReturn(
        keyCode: UInt16 = 36, characters: String = "\r", flags: NSEvent.ModifierFlags = [],
        text: String = "\\sec", caret: Int = 4, selectNext: Bool = false,
        label: String = "\\section", inserted: String = "\\section{}", newline: String = "\\section{\n}"
    ) throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-completion-keyboard-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data(text.utf8).write(to: root.appendingPathComponent("main.tex"))
        try Data("\\newcommand{\\myfirst}[1]{#1}\n\\newcommand{\\mysecond}[1]{#1}\n\\label{sec:methods}".utf8).write(to: root.appendingPathComponent("definitions.tex"))
        try Data("@article{knuth1984, title={The Book}}\n@book{smith2026, title={Research}}".utf8).write(to: root.appendingPathComponent("refs.bib"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        let window = NSWindow(contentRect: NSRect(x: -6000, y: -6000, width: 700, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = scroll
        defer {
            coordinator.completion.dismiss(); window.contentView = nil; window.close()
            store.save(); XCTAssertTrue(store.requestTermination())
            try? FileManager.default.removeItem(at: root)
        }
        editor.synchronize(scroll, coordinator: coordinator)
        let source = try XCTUnwrap(scroll.documentView as? SourceTextView)
        XCTAssertTrue(window.makeFirstResponder(source))
        source.setSelectedRange(NSRange(location: caret, length: 0))
        coordinator.completion.refresh(source)
        let popup = try XCTUnwrap(window.childWindows?.first)
        XCTAssertTrue(popup.isVisible)
        if selectNext {
            let down = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.function, .numericPad], timestamp: 0,
                windowNumber: window.windowNumber, context: nil, characters: "\u{F701}", charactersIgnoringModifiers: "\u{F701}", isARepeat: false, keyCode: 125))
            window.sendEvent(down)
        }
        let proposal = try XCTUnwrap(coordinator.completion.proposal)
        XCTAssertEqual(proposal.items[coordinator.completion.selectedIndex].label, label)
        let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: keyCode))
        window.sendEvent(event)
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(store.text, inserted)
        XCTAssertEqual(source.string, inserted)
        XCTAssertNil(coordinator.completion.proposal)
        XCTAssertTrue(window.childWindows?.isEmpty ?? true)
        XCTAssertFalse(popup.isVisible)
        XCTAssertTrue(window.firstResponder === source)
        XCTAssertEqual(store.revision, text == inserted ? 1 : 2)
        window.sendEvent(event)
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(store.text, newline)
        XCTAssertNil(coordinator.completion.proposal)
        XCTAssertTrue(window.childWindows?.isEmpty ?? true)
        store.undo(); XCTAssertEqual(store.text, inserted)
        if text != inserted { store.undo() }
        XCTAssertEqual(store.text, text); XCTAssertFalse(store.canUndo)
    }

    @MainActor func testReturnAcceptsOnceClosesPopupAndThenInsertsNewline() throws {
        try checkReturn(keyCode: 36, characters: "\r")
    }

    @MainActor func testKeypadEnterAcceptsOnceClosesPopupAndThenInsertsNewline() throws {
        try checkReturn(keyCode: 76, characters: "\u{3}", flags: .numericPad)
    }

    @MainActor func testArrowSelectedMacroReturnHidesTheActualPopupAndKeepsSourceFocus() throws {
        try checkReturn(text: "\\my", caret: 3, selectNext: true, label: "\\mysecond", inserted: "\\mysecond{}", newline: "\\mysecond{\n}")
    }

    @MainActor func testAlreadyCompleteReferenceReturnClosesWithoutAddingHistory() throws {
        try checkReturn(text: "\\ref{sec:methods}", caret: 16, label: "sec:methods", inserted: "\\ref{sec:methods}", newline: "\\ref{sec:methods\n}")
    }

    @MainActor func testCitationReturnHidesThePopupThroughPendingRefresh() throws {
        try checkReturn(text: "\\cite{knuth1984, s}", caret: 18, label: "smith2026", inserted: "\\cite{knuth1984, smith2026}", newline: "\\cite{knuth1984, smith2026\n}")
    }
}
