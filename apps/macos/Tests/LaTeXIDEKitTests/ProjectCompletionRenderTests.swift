import AppKit
import XCTest
@testable import LaTeXIDEKit

/// Actual core candidates and native popup, off-screen; no mocked language service.
final class ProjectCompletionRenderTests: XCTestCase {
    @MainActor func testRenderProjectMacroReferenceAndCitationCandidates() throws {
        guard ProcessInfo.processInfo.environment["LATEXIDE_RENDER_PREVIEWS"] == "1" else { throw XCTSkip("Opt-in native project completion render") }
        _ = NSApplication.shared
        var repository = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repository.deleteLastPathComponent() }
        let output = repository.appendingPathComponent("evidence/project-completion-2026-10-31")
        try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-project-render-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let files = [
            "main.tex": "",
            "macros.tex": "\\newcommand{\\myexpectation}[1]{\\mathbb{E}[#1]}\n\\newcommand{\\myvariance}[1]{\\mathrm{Var}(#1)}\n\\NewDocumentCommand{\\mynorm}{m}{\\lVert#1\\rVert}",
            "methods.tex": "\\label{sec:methods}\n\\label{sec:results}\n\\label{sec:discussion}",
            "references.bib": "@article{knuth1984, title={Literate Programming}}\n@book{lamport1994, title={LaTeX}}\n@article{smith2026, title={Research Methods}}",
            "macro-example.tex": "\\my", "reference-example.tex": "\\ref{sec:}", "citation-example.tex": "\\cite{c}"
        ]
        for (path, text) in files { try Data(text.utf8).write(to: root.appendingPathComponent(path)) }
        let store = WorkspaceStore(); store.open(root)
        let theme = ThemeCatalog.shared.palette("graphite").resolve(dark: true)
        let window = NSWindow(contentRect: NSRect(x: -6000,y: -6000,width: 700,height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.appearance = NSAppearance(named: .darkAqua)
        defer { window.close() }
        for (kind, path, caret, expected) in [
            ("macro", "macro-example.tex", 3, "\\myexpectation"),
            ("reference", "reference-example.tex", 9, "sec:discussion"),
            ("citation", "citation-example.tex", 6, "knuth1984")
        ] {
            // The citation example is changed before opening so its empty key lists all entries.
            if kind == "citation" { try Data("\\cite{}".utf8).write(to: root.appendingPathComponent(path)) }
            store.select(path)
            let editor = NativeEditor(store: store, theme: theme), coordinator = editor.makeCoordinator()
            let scroll = editor.makeScrollView(coordinator: coordinator); window.contentView = scroll
            editor.synchronize(scroll, coordinator: coordinator)
            let source = try XCTUnwrap(scroll.documentView as? SourceTextView)
            window.makeFirstResponder(source); source.setSelectedRange(NSRange(location: caret,length: 0))
            coordinator.completion.refresh(source)
            XCTAssertEqual(coordinator.completion.proposal?.items.first?.label, expected)
            XCTAssertEqual(coordinator.completion.proposal?.items.first?.kind, kind)
            let popup = try XCTUnwrap(window.childWindows?.first), view = try XCTUnwrap(popup.contentView)
            view.layoutSubtreeIfNeeded(); RunLoop.current.run(until: Date().addingTimeInterval(0.1)); view.layoutSubtreeIfNeeded()
            let bitmap = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
            view.cacheDisplay(in: view.bounds, to: bitmap)
            let png = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
            try png.write(to: output.appendingPathComponent("project-\(kind)-dark.png"))
            coordinator.completion.dismiss()
        }
        XCTAssertFalse(store.canUndo); XCTAssertTrue(store.requestTermination())
        try Data("Actual core-backed native macro/reference/citation popups. Off-screen component renders, not live screenshots; no model or mocked candidates.\n".utf8).write(to: output.appendingPathComponent("render-scope.txt"))
    }
}
