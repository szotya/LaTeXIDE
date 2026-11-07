import AppKit
import SwiftUI
import XCTest
@testable import LaTeXIDEKit

private final class EditorVisibility: ObservableObject {
    @Published var visible = true
}
private struct HostedSourceEditor: View {
    let store: WorkspaceStore
    @ObservedObject var visibility: EditorVisibility
    var body: some View {
        Group {
            if visibility.visible { NativeEditor(store: store) }
            else { Color.clear }
        }
    }
}
final class WorkspaceObservationTests: XCTestCase {
    @MainActor private func fixture(_ source: String) throws -> (URL, WorkspaceStore) {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-observation-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try Data(source.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        XCTAssertNil(store.failure)
        return (root, store)
    }
    @MainActor private func source(_ root: NSView) -> SourceTextView? {
        if let view = root as? SourceTextView { return view }
        return root.subviews.lazy.compactMap { self.source($0) }.first
    }
    @MainActor private func settle(_ host: NSView) async throws {
        host.layoutSubtreeIfNeeded()
        try await Task.sleep(nanoseconds: 80_000_000)
        host.layoutSubtreeIfNeeded(); host.window?.displayIfNeeded()
    }
    @MainActor private func host(_ store: WorkspaceStore, visibility: EditorVisibility) -> NSHostingView<HostedSourceEditor> {
        NSHostingView(rootView: HostedSourceEditor(store: store, visibility: visibility))
    }
    @MainActor private func window(_ host: NSView) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: -6000, y: -6000, width: 500, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = host
        return window
    }
    @MainActor func testExternalCanonicalEditAutomaticallyRefreshesHostedEditorAndNextUTF16Edit() async throws {
        let (root, store) = try fixture("é tail")
        let host = host(store, visibility: EditorVisibility()), window = window(host)
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        try await settle(host)
        let view = try XCTUnwrap(source(host))
        try store.edit(range: NSRange(location: 0, length: 1), replacement: "e\u{301}")
        try await settle(host)
        XCTAssertTrue(view.hasLiteralText("e\u{301} tail"))
        XCTAssertEqual(store.revision, 2)
        XCTAssertTrue(window.makeFirstResponder(view))
        view.insertText("🙂", replacementRange: NSRange(location: 2, length: 0))
        try await settle(host)
        XCTAssertTrue(view.hasLiteralText("e\u{301}🙂 tail")); XCTAssertTrue(view.hasLiteralText(store.text))
        XCTAssertEqual(store.revision, 3); XCTAssertNil(store.failure)
        store.undo(); try await settle(host)
        XCTAssertTrue(view.hasLiteralText("e\u{301} tail"))
    }
    @MainActor func testHostedDestructionAndRecreationRestoresPresentationWithoutManualSynchronization() async throws {
        let (root, store) = try fixture((0..<200).map { "Sor \($0) · Árvíz 😀" }.joined(separator: "\n"))
        let visibility = EditorVisibility(), host = host(store, visibility: visibility), window = window(host)
        defer { window.close(); try? FileManager.default.removeItem(at: root) }
        try await settle(host)
        let first = try XCTUnwrap(source(host)), selection = NSRange(location: 4, length: 10)
        first.setSelectedRange(selection)
        let clip = try XCTUnwrap(first.enclosingScrollView?.contentView)
        clip.scroll(to: NSPoint(x: 0, y: 850)); let origin = clip.bounds.origin
        XCTAssertEqual(try XCTUnwrap(store.editorStates["main.tex"]).scrollOrigin.y, origin.y, accuracy: 0.5, "live capture")
        visibility.visible = false; try await settle(host)
        XCTAssertEqual(try XCTUnwrap(store.editorStates["main.tex"]).scrollOrigin.y, origin.y, accuracy: 0.5, "hidden capture")
        XCTAssertNil(source(host))
        visibility.visible = true; try await settle(host)
        let recreated = try XCTUnwrap(source(host))
        XCTAssertFalse(recreated === first)
        XCTAssertEqual(recreated.selectedRange(), selection)
        XCTAssertEqual(try XCTUnwrap(recreated.enclosingScrollView).contentView.bounds.origin.y, origin.y, accuracy: 0.5)
        XCTAssertTrue(recreated.hasLiteralText(store.text)); XCTAssertFalse(store.canUndo)
    }
    @MainActor func testSameRevisionSaveUpdatesPresentedDirtyAndHiddenMetadata() throws {
        let (root, store) = try fixture("owned source")
        defer { try? FileManager.default.removeItem(at: root) }
        try store.edit(range: NSRange(location: 0, length: 1), replacement: "O")
        let revision = store.revision
        XCTAssertTrue(try XCTUnwrap(store.presentedTabs.first).dirty)
        store.save()
        XCTAssertEqual(store.revision, revision); XCTAssertFalse(store.dirty)
        XCTAssertFalse(try XCTUnwrap(store.presentedTabs.first).dirty)
        XCTAssertFalse(try XCTUnwrap(store.presentedDocuments.first).dirty)
        store.hideTab("main.tex")
        XCTAssertTrue(store.presentedTabs.isEmpty); XCTAssertEqual(store.hiddenTabCount, 1)
        store.select("main.tex")
        XCTAssertEqual(store.hiddenTabCount, 0); XCTAssertEqual(store.revision, revision)
        XCTAssertFalse(try XCTUnwrap(store.presentedTabs.first).dirty)
        XCTAssertNil(store.failure)
    }
}
