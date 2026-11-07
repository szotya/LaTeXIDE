import AppKit
import XCTest
@testable import LaTeXIDEKit

final class RecoveryStatusTests: XCTestCase {
    @MainActor private func fixture() throws -> (URL, WorkspaceStore, String) {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-recovery-warning-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        addTeardownBlock { try FileManager.default.removeItem(at: root) }
        try Data("A\n".utf8).write(to: root.appendingPathComponent("main.tex"))
        let metadata = root.appendingPathComponent(".latexide")
        try FileManager.default.createDirectory(at: metadata, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        try Data("owned recovery setup blocker".utf8).write(to: metadata.appendingPathComponent("recovery"))
        let expectedError: String
        do {
            let probe = try NativeRuntime(root: root)
            let status = try XCTUnwrap(try probe.call("workspace_info")["recovery"] as? [String: Any])
            XCTAssertEqual(status["enabled"] as? Bool, false)
            expectedError = try XCTUnwrap(status["last_error"] as? String)
        }
        let store = WorkspaceStore()
        addTeardownBlock {
            await MainActor.run {
                store.save(); XCTAssertTrue(store.requestTermination())
            }
        }
        return (root, store, expectedError)
    }

    @MainActor func testSetupFailureWarnsBeforeEditingAndPreservesMemorySaveAndErrors() throws {
        let (root, store, code) = try fixture()
        store.open(root)
        XCTAssertNil(store.failure); XCTAssertEqual(store.selection, "main.tex")
        let warning = try XCTUnwrap(store.recoveryWarning)
        XCTAssertTrue(warning.contains(code), "The actual runtime last_error must be available to the user")
        XCTAssertTrue(warning.contains("mentetlen"))
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        XCTAssertTrue(view.isEditable, "The explicit in-memory fallback must remain usable after the warning")
        view.insertText("B", replacementRange: NSRange(location: 0, length: 1))
        XCTAssertEqual(store.text, "B\n"); XCTAssertTrue(store.dirty); XCTAssertTrue(store.canUndo)
        XCTAssertEqual(store.recoveryWarning, warning)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data("A\n".utf8))
        store.failure = "An unrelated operation failed"
        try store.refreshInfo()
        XCTAssertEqual(store.failure, "An unrelated operation failed")
        XCTAssertEqual(store.recoveryWarning, warning)
        store.failure = nil; store.save()
        XCTAssertNil(store.failure); XCTAssertFalse(store.dirty)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data("B\n".utf8))
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent(".latexide/recovery")), Data("owned recovery setup blocker".utf8))
    }

    @MainActor func testOpeningHealthyProjectClearsPreviousRecoveryWarning() throws {
        let (root, store, _) = try fixture()
        store.open(root); XCTAssertNotNil(store.recoveryWarning)
        let healthy = root.appendingPathComponent("healthy")
        try FileManager.default.createDirectory(at: healthy, withIntermediateDirectories: false)
        try Data("Healthy\n".utf8).write(to: healthy.appendingPathComponent("main.tex"))
        store.open(healthy, discardCurrent: true)
        XCTAssertNil(store.failure); XCTAssertNil(store.recoveryWarning)
        XCTAssertEqual(store.root, healthy); XCTAssertEqual(store.text, "Healthy\n")
        try store.edit(range: NSRange(location: 0, length: 1), replacement: "h")
        XCTAssertNil(store.recoveryWarning); XCTAssertTrue(store.dirty)
    }
}
