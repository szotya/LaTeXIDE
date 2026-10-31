import AppKit
import XCTest
@testable import LaTeXIDEKit

final class DefinitionNavigationTests: XCTestCase {
    @MainActor private final class Fixture {
        let root: URL
        let store: WorkspaceStore
        let editor: NativeEditor
        let coordinator: NativeEditor.Coordinator
        let scroll: NSScrollView
        let source: SourceTextView
        let window: NSWindow
        init(_ text: String = "😀 \\mean{x} \\ref{sec:methods} \\citep[see]{other, smith2026}") throws {
            _ = NSApplication.shared
            root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-definition-native-\(UUID().uuidString)")
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
            try Data(text.utf8).write(to: root.appendingPathComponent("main.tex"))
            try Data("😀\n\\newcommand{\\mean}[1]{#1}\n\\label{sec:methods}\n".utf8).write(to: root.appendingPathComponent("defs.tex"))
            try Data("@article{other, title={Other}}\n@book{smith2026, title={Research}}\n".utf8).write(to: root.appendingPathComponent("refs.bib"))
            store = WorkspaceStore(); store.open(root)
            editor = NativeEditor(store: store); coordinator = editor.makeCoordinator()
            scroll = editor.makeScrollView(coordinator: coordinator)
            window = NSWindow(contentRect: NSRect(x: -6000, y: -6000, width: 900, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; window.contentView = scroll
            editor.synchronize(scroll, coordinator: coordinator)
            source = try XCTUnwrap(scroll.documentView as? SourceTextView)
            window.makeFirstResponder(source); source.layoutManager?.ensureLayout(for: source.textContainer!)
        }
        func cleanup() {
            coordinator.completion.dismiss(); window.contentView = nil; window.close()
            store.save(); XCTAssertTrue(store.requestTermination()); try? FileManager.default.removeItem(at: root)
        }
        func event(on substring: String, flags: NSEvent.ModifierFlags = .command, point: NSPoint? = nil) throws -> NSEvent {
            let range = (source.string as NSString).range(of: substring); XCTAssertNotEqual(range.location, NSNotFound)
            let manager = try XCTUnwrap(source.layoutManager), container = try XCTUnwrap(source.textContainer)
            let glyphs = manager.glyphRange(forCharacterRange: NSRange(location: range.location, length: 1), actualCharacterRange: nil)
            let rect = manager.boundingRect(forGlyphRange: glyphs, in: container)
            let local = point ?? NSPoint(x: rect.midX + source.textContainerOrigin.x, y: rect.midY + source.textContainerOrigin.y)
            let location = source.convert(local, to: nil)
            return try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: location, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
        }
        func click(_ text: String) throws {
            let down = try event(on: text)
            // Queue only this test's mouse-up so a missing handler falls through safely.
            let up = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseUp, location: down.locationInWindow, modifierFlags: down.modifierFlags, timestamp: 0.01, windowNumber: window.windowNumber, context: nil, eventNumber: 2, clickCount: 1, pressure: 0))
            NSApp.postEvent(up, atStart: false); source.mouseDown(with: down)
            _ = NSApp.nextEvent(matching: .leftMouseUp, until: Date(), inMode: .default, dequeue: true)
            editor.synchronize(scroll, coordinator: coordinator)
        }
    }

    @MainActor func testCommandClickMacroReferenceAndCitationSelectExactDefinitionsWithoutEdits() throws {
        for (clicked, file, selected) in [("\\mean", "defs.tex", "\\mean"), ("sec:methods", "defs.tex", "sec:methods"), ("smith2026", "refs.bib", "smith2026")] {
            let f = try Fixture(); defer { f.cleanup() }
            let baseline = f.store.text
            f.source.setSelectedRange(NSRange(location: 2, length: 0)); f.coordinator.capturePresentation(f.source)
            try f.click(clicked)
            XCTAssertEqual(f.store.selection, file)
            XCTAssertEqual((f.source.string as NSString).substring(with: f.source.selectedRange()), selected)
            XCTAssertTrue(f.window.firstResponder === f.source)
            XCTAssertFalse(f.store.canUndo); XCTAssertEqual(f.store.revision, 1); XCTAssertFalse(f.store.dirty)
            XCTAssertTrue(f.store.navigationNotice?.contains(file) == true)
            f.store.select("main.tex"); f.editor.synchronize(f.scroll, coordinator: f.coordinator)
            XCTAssertEqual(f.source.string, baseline); XCTAssertEqual(f.source.selectedRange(), NSRange(location: 2, length: 0))
        }
    }

    @MainActor func testCommandClickFinalMacroCharacterBeforeCommentResolves() throws {
        let f = try Fixture("\\mean% comment"); defer { f.cleanup() }
        try f.click("n%")
        XCTAssertEqual(f.store.selection, "defs.tex")
        XCTAssertEqual((f.source.string as NSString).substring(with: f.source.selectedRange()), "\\mean")
        XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testDirtyTargetAndUndoDetermineTheActualSelectedRange() throws {
        let f = try Fixture(); defer { f.cleanup() }
        f.store.select("defs.tex")
        try f.store.edit(range: NSRange(location: 0, length: 0), replacement: "Dirty line\n")
        f.store.select("main.tex"); f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        try f.click("\\mean")
        XCTAssertEqual(f.store.selection, "defs.tex"); XCTAssertTrue(f.store.dirty)
        XCTAssertEqual(f.source.selectedRange(), NSRange(location: 26, length: 5))
        f.store.undo(); XCTAssertEqual(f.store.text, "😀\n\\newcommand{\\mean}[1]{#1}\n\\label{sec:methods}\n")
        XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testUnopenedBOMCRLFTargetSelectsTheNormalizedName() throws {
        let f = try Fixture(); defer { f.cleanup() }
        try Data("\u{feff}😀\r\n\\newcommand{\\mean}[1]{}\r\n".utf8).write(to: f.root.appendingPathComponent("defs.tex"))
        try f.click("\\mean")
        XCTAssertEqual(f.store.selection, "defs.tex"); XCTAssertTrue(f.store.bom); XCTAssertEqual(f.store.lineEnding, "crlf")
        XCTAssertEqual(f.source.selectedRange(), NSRange(location: 15, length: 5))
    }

    @MainActor func testDefinitionFarBelowTheViewportIsScrolledIntoView() throws {
        let f = try Fixture(); defer { f.cleanup() }
        try Data((String(repeating: "Research line\n", count: 200) + "\\newcommand{\\mean}[1]{}").utf8).write(to: f.root.appendingPathComponent("defs.tex"))
        try f.click("\\mean")
        XCTAssertEqual(f.store.selection, "defs.tex")
        XCTAssertEqual(f.source.selectedRange(), NSRange(location: 2812, length: 5))
        let manager = try XCTUnwrap(f.source.layoutManager), container = try XCTUnwrap(f.source.textContainer)
        let glyphs = manager.glyphRange(forCharacterRange: f.source.selectedRange(), actualCharacterRange: nil)
        let rect = manager.boundingRect(forGlyphRange: glyphs, in: container).offsetBy(dx: f.source.textContainerOrigin.x, dy: f.source.textContainerOrigin.y)
        XCTAssertTrue(f.scroll.documentVisibleRect.intersects(rect))
        XCTAssertGreaterThan(f.scroll.contentView.bounds.origin.y, 0)
    }

    @MainActor func testRecreatedEditorDoesNotReplayAnAlreadyAppliedDefinitionJump() throws {
        let f = try Fixture(); defer { f.cleanup() }
        try Data((String(repeating: "Research line\n", count: 200) + "\\newcommand{\\mean}[1]{}").utf8).write(to: f.root.appendingPathComponent("defs.tex"))
        try f.click("\\mean")
        f.source.setSelectedRange(NSRange(location: 0, length: 0))
        f.scroll.contentView.scroll(to: .zero); f.coordinator.capturePresentation(f.source)
        f.store.hideTab("main.tex"); f.store.hideTab("defs.tex")
        NativeEditor.dismantleNSView(f.scroll, coordinator: f.coordinator); f.window.contentView = nil
        f.store.select("defs.tex")
        let editor = NativeEditor(store: f.store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator); f.window.contentView = scroll
        editor.synchronize(scroll, coordinator: coordinator)
        let source = try XCTUnwrap(scroll.documentView as? SourceTextView)
        XCTAssertEqual(source.selectedRange(), NSRange(location: 0, length: 0))
        XCTAssertEqual(scroll.contentView.bounds.origin.y, 0, accuracy: 0.5)
        XCTAssertFalse(f.store.canUndo)
        NativeEditor.dismantleNSView(scroll, coordinator: coordinator)
    }

    @MainActor func testPlainModifiedClicksAndBlankSpaceDoNotResolveDefinitions() throws {
        let f = try Fixture(); defer { f.cleanup() }
        for flags: NSEvent.ModifierFlags in [[], [.command, .shift], [.command, .option], [.command, .control]] {
            XCTAssertFalse(f.source.followDefinition(with: try f.event(on: "\\mean", flags: flags)))
        }
        XCTAssertFalse(f.source.followDefinition(with: try f.event(on: "\\mean", point: NSPoint(x: 850, y: 70))))
        XCTAssertFalse(f.source.followDefinition(with: try f.event(on: "\\mean", point: NSPoint(x: 2, y: 3))))
        XCTAssertEqual(f.store.selection, "main.tex"); XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testMarkedTextAndStaleRenderedSourceDoNotNavigate() throws {
        let f = try Fixture(); defer { f.cleanup() }
        let click = try f.event(on: "\\mean")
        try f.store.edit(range: NSRange(location: 0, length: 0), replacement: "new")
        XCTAssertFalse(f.source.followDefinition(with: click)); XCTAssertEqual(f.store.selection, "main.tex")
        f.store.undo(); f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        f.source.setMarkedText("日", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 0, length: 0))
        XCTAssertFalse(f.source.followDefinition(with: click)); XCTAssertEqual(f.store.selection, "main.tex")
        f.source.unmarkText(); f.store.undo()
    }

    @MainActor func testMissingDefinitionLeavesSourceAndHistoryIntactWithNonmodalFeedback() throws {
        let f = try Fixture("\\missing"); defer { f.cleanup() }
        XCTAssertFalse(f.store.goToDefinition(at: 2))
        XCTAssertEqual(f.store.selection, "main.tex"); XCTAssertEqual(f.store.text, "\\missing")
        XCTAssertNil(f.store.failure); XCTAssertNotNil(f.store.navigationNotice); XCTAssertFalse(f.store.canUndo)
    }

    @MainActor func testNavigationRangeIsNotAppliedAfterTargetRevisionChanges() throws {
        let f = try Fixture(); defer { f.cleanup() }
        XCTAssertTrue(f.store.goToDefinition(at: 4)); XCTAssertEqual(f.store.selection, "defs.tex")
        try f.store.edit(range: NSRange(location: 0, length: 0), replacement: "changed\n")
        f.editor.synchronize(f.scroll, coordinator: f.coordinator)
        XCTAssertNotEqual(f.source.selectedRange(), NSRange(location: 15, length: 5))
    }

    @MainActor func testOpenDocumentLimitDoesNotSwitchAwayFromTheSource() throws {
        let f = try Fixture(); defer { f.cleanup() }
        for index in 0..<9 {
            let name = "other\(index).tex"; try Data("text".utf8).write(to: f.root.appendingPathComponent(name)); f.store.select(name)
        }
        f.store.select("main.tex"); XCTAssertFalse(f.store.goToDefinition(at: 4))
        XCTAssertEqual(f.store.selection, "main.tex"); XCTAssertFalse(f.store.canUndo)
        XCTAssertNotNil(f.store.navigationNotice); XCTAssertNil(f.store.failure)
    }
}
