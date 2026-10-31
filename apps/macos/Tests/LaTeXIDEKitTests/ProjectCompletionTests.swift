import AppKit
import XCTest
@testable import LaTeXIDEKit

final class ProjectCompletionTests: XCTestCase {
    @MainActor private func fixture(_ marked: String) throws -> (URL, WorkspaceStore, NativeEditor.Coordinator, SourceTextView) {
        _ = NSApplication.shared
        let parts = marked.components(separatedBy: "<caret>"); XCTAssertEqual(parts.count, 2)
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-project-candidates-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data(parts.joined().utf8).write(to: root.appendingPathComponent("main.tex"))
        try Data("\\newcommand{\\mean}[2]{#1+#2}".utf8).write(to: root.appendingPathComponent("macros.tex"))
        try Data("\\label{sec:overview}".utf8).write(to: root.appendingPathComponent("labels.tex"))
        try Data("@book{knuth1984, title={The Book}}\n@article{smith2026, title={Research}}".utf8).write(to: root.appendingPathComponent("refs.bib"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator); scroll.frame = NSRect(x: 0,y: 0,width: 700,height: 300)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        view.setSelectedRange(NSRange(location: (parts[0] as NSString).length, length: 0))
        return (root, store, coordinator, view)
    }
    @MainActor private func checkInsertion(_ marked: String, expected: String, caret: Int) throws {
        let (root, store, coordinator, view) = try fixture(marked)
        defer { try? FileManager.default.removeItem(at: root) }
        let baseline = store.text
        coordinator.completion.refresh(view)
        XCTAssertNotNil(coordinator.completion.proposal)
        XCTAssertTrue(coordinator.completion.handle(#selector(NSResponder.insertTab(_:)), view: view))
        XCTAssertEqual(store.text, expected); XCTAssertEqual(view.string, expected)
        XCTAssertEqual(view.selectedRange(), NSRange(location: caret, length: 0)); XCTAssertEqual(store.revision, 2)
        store.undo(); XCTAssertEqual(store.text, baseline); XCTAssertFalse(store.canUndo)
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testProjectMacroInsertsArgumentsAsOneSharedUndo() throws {
        try checkInsertion("\\mea<caret>", expected: "\\mean{}{}", caret: 6)
    }
    @MainActor func testReferenceReplacesTheKeyAfterAnEmojiAndKeepsClosingBrace() throws {
        try checkInsertion("😀 \\ref{sec:o<caret>ZZ}", expected: "😀 \\ref{sec:overview}", caret: 20)
    }
    @MainActor func testCitationKeepsOptionalArgumentsAndOtherCitationKeys() throws {
        try checkInsertion("\\citep[see]{alpha, kn<caret>ZZ, omega}", expected: "\\citep[see]{alpha, knuth1984, omega}", caret: 28)
    }
    @MainActor func testWhitespaceAfterCitationCommaOffersUnusedKeys() throws {
        try checkInsertion("\\cite{knuth1984, <caret>}", expected: "\\cite{knuth1984, smith2026}", caret: 26)
    }
    @MainActor func testEarlierUnfinishedCitationDoesNotCloseBeforeLaterKeys() throws {
        try checkInsertion("\\cite{kn<caret>, omega", expected: "\\cite{knuth1984, omega", caret: 15)
    }
    @MainActor func testAcceptingAnAlreadyCompleteKeyConsumesTabWithoutAnEdit() throws {
        let (root, store, coordinator, view) = try fixture("\\ref{sec:overview<caret>}")
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        XCTAssertNotNil(coordinator.completion.proposal)
        let handled = coordinator.completion.handle(#selector(NSResponder.insertTab(_:)), view: view)
        if !handled { view.insertTab(nil) }
        XCTAssertTrue(handled)
        XCTAssertEqual(store.text, "\\ref{sec:overview}"); XCTAssertEqual(store.revision, 1)
        XCTAssertFalse(store.canUndo); XCTAssertEqual(view.selectedRange(), NSRange(location: 17,length: 0))
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testAcceptedKeyStaysDismissedUntilTheContextChanges() throws {
        let (root, store, coordinator, view) = try fixture("\\ref{sec:o<caret>}")
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        XCTAssertTrue(coordinator.completion.accept(view))
        // Pending text/selection notifications can request the exact same query after acceptance.
        coordinator.completion.refresh(view)
        XCTAssertNil(coordinator.completion.proposal)
        view.setSelectedRange(NSRange(location: 9,length: 0))
        coordinator.completion.refresh(view)
        XCTAssertNotNil(coordinator.completion.proposal)
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testChangedOtherDocumentRejectsStaleCandidateAndLetsReturnThrough() throws {
        let (root, store, coordinator, view) = try fixture("\\ref{sec:<caret>}")
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        XCTAssertEqual(coordinator.completion.proposal?.items.first?.label, "sec:overview")
        store.select("labels.tex")
        try store.edit(range: NSRange(location: 0,length: (store.text as NSString).length), replacement: "\\label{sec:changed}")
        store.select("main.tex")
        XCTAssertEqual(store.revision, 1)
        XCTAssertFalse(coordinator.completion.handle(#selector(NSResponder.insertNewline(_:)), view: view))
        XCTAssertEqual(store.text, "\\ref{sec:}"); XCTAssertNil(coordinator.completion.proposal)
        store.undo(); XCTAssertEqual(store.text, "\\label{sec:overview}")
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testChangedDiskDefinitionForcesFreshValidationBeforeTabAcceptance() throws {
        let (root, store, coordinator, view) = try fixture("\\ref{sec:<caret>}")
        defer { try? FileManager.default.removeItem(at: root) }
        coordinator.completion.refresh(view)
        XCTAssertEqual(coordinator.completion.proposal?.items.first?.label, "sec:overview")
        try Data("\\label{sec:changed}".utf8).write(to: root.appendingPathComponent("labels.tex"))
        XCTAssertFalse(coordinator.completion.handle(#selector(NSResponder.insertTab(_:)), view: view))
        XCTAssertEqual(store.text, "\\ref{sec:}"); XCTAssertFalse(store.canUndo)
        store.save(); XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testPartialIndexStateIsVisibleEvenWhenNoCandidateMatches() throws {
        let (root, store, _, view) = try fixture("\\ref{zzNoMatching<caret>}")
        defer { try? FileManager.default.removeItem(at: root) }
        for index in 0..<130 {
            try Data("\\label{key:\(index)}".utf8).write(to: root.appendingPathComponent("chapter\(index).tex"))
        }
        XCTAssertNil(store.completions(caret: view.selectedRange().location))
        XCTAssertEqual(store.completionIndexStatus?.truncated, true)
        XCTAssertEqual(store.revision, 1); XCTAssertFalse(store.canUndo)
        XCTAssertTrue(store.requestTermination())
    }
}
