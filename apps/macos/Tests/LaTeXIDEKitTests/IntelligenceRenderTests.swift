import XCTest
import AppKit
import SwiftUI
@testable import LaTeXIDEKit

/// Actual native components, off-screen controlled render; no model call or fabricated transcript.
final class IntelligenceRenderTests: XCTestCase {
    @MainActor func testRenderAssistantAndCompletionControls() throws {
        guard ProcessInfo.processInfo.environment["LATEXIDE_RENDER_PREVIEWS"] == "1" else { throw XCTSkip("Opt-in native visual artifacts") }
        _ = NSApplication.shared
        var repository = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repository.deleteLastPathComponent() }
        let output = repository.appendingPathComponent("evidence/intelligence-2026-10-31")
        try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
        let project = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-intelligence-render-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: project, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: project) }
        try Data("\\s".utf8).write(to: project.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(project)
        let session = AssistantSession()
        session.configure(root: project, sessionPath: store.sessionPath, bridgeURL: repository.appendingPathComponent("target/debug/latexide-mcp"))
        store.inspector = .assistant
        let theme = ThemeCatalog.shared.palette("graphite").resolve(dark: true)
        let host = NSHostingView(rootView: InspectorPanel(store: store, session: session).environment(\.ideTheme, theme).preferredColorScheme(.dark).frame(width: 440, height: 900))
        host.frame = NSRect(x: 0,y: 0,width: 440,height: 900)
        let window = NSWindow(contentRect: NSRect(x: -6000,y: -6000,width: 440,height: 900), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.appearance = NSAppearance(named: .darkAqua)
        window.contentView = host; window.orderBack(nil)
        defer { window.close(); session.disconnect() }
        try render(host, to: output.appendingPathComponent("assistant-panel-dark.png"))

        let editor = NativeEditor(store: store, theme: theme)
        let coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        let source = scroll.documentView as! SourceTextView
        window.contentView = scroll
        window.setContentSize(NSSize(width: 700, height: 350))
        editor.synchronize(scroll, coordinator: coordinator)
        window.makeFirstResponder(source); source.setSelectedRange(NSRange(location: 2, length: 0))
        coordinator.completion.refresh(source)
        let popup = try XCTUnwrap(window.childWindows?.first)
        try render(try XCTUnwrap(popup.contentView), to: output.appendingPathComponent("command-completion-dark.png"))
        coordinator.completion.dismiss()
        XCTAssertEqual(store.text, "\\s"); XCTAssertEqual(store.revision, 1); XCTAssertFalse(store.canUndo)
        XCTAssertTrue(store.requestTermination())
        let scope = "Actual InspectorPanel/AssistantPanel idle state and native command popup. No model connection or generated reply; off-screen component renders, not live screenshots."
        try Data(scope.utf8).write(to: output.appendingPathComponent("component-render-scope.txt"))
    }
    @MainActor private func render(_ view: NSView, to url: URL) throws {
        view.layoutSubtreeIfNeeded()
        RunLoop.current.run(until: Date().addingTimeInterval(0.15))
        view.layoutSubtreeIfNeeded(); view.displayIfNeeded()
        let bitmap = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: bitmap)
        let png = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
        try png.write(to: url); XCTAssertGreaterThan(png.count, 10000)
    }
}
