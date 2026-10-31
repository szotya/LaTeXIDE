import XCTest
@testable import LaTeXIDEKit

final class WorkspaceTabsTests: XCTestCase {
    @MainActor func testHidingDirtyTabRetainsTextAndSharedUndo() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-tabs-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("main".utf8).write(to: root.appendingPathComponent("main.tex"))
        try Data("chapter".utf8).write(to: root.appendingPathComponent("chapter.tex"))
        let store = WorkspaceStore(); store.open(root)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "unsaved ")
        store.select("chapter.tex")
        store.hideTab("main.tex")
        XCTAssertFalse(store.tabs.contains(where: { $0.id == "main.tex" }))
        XCTAssertTrue(store.retainedDocuments.contains(where: { $0.id == "main.tex" && $0.dirty }))
        store.select("main.tex")
        XCTAssertEqual(store.text, "unsaved main")
        XCTAssertTrue(store.dirty)
        store.undo()
        XCTAssertEqual(store.text, "main")
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor func testHidingLastTabCanBeRecoveredByUndo() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-hidden-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("main".utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        try store.edit(range: NSRange(location: 0, length: 0), replacement: "x")
        store.hideTab("main.tex")
        XCTAssertNil(store.selection)
        XCTAssertEqual(store.hiddenTabCount, 1)
        store.undo()
        XCTAssertEqual(store.selection, "main.tex")
        XCTAssertEqual(store.text, "main")
        XCTAssertTrue(store.requestTermination())
    }
}
