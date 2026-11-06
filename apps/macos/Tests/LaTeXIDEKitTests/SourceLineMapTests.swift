import XCTest
import AppKit
@testable import LaTeXIDEKit

final class SourceLineMapTests: XCTestCase {
    func testIncrementalEditsMatchFreshLineMapsAtUTF16Boundaries() {
        for original in ["", "a\nb\n", "😀e\u{301}\nα\nend", "\n\n"] {
            for replacement in ["", "x", "😀", "\n", "p\nq\n"] {
                let units: [UInt16] = Array(original.utf16)
                var boundaries: [Int] = []
                for offset in 0...units.count {
                    if offset == 0 || offset == units.count {
                        boundaries.append(offset)
                        continue
                    }
                    let previous = units[offset - 1], next = units[offset]
                    let high = previous >= 0xD800 && previous <= 0xDBFF
                    let low = next >= 0xDC00 && next <= 0xDFFF
                    if !high || !low { boundaries.append(offset) }
                }
                for start in boundaries {
                    for end in boundaries where end >= start {
                        let range = NSRange(location: start, length: end - start)
                        var map = SourceLineMap(original)
                        XCTAssertTrue(map.replace(range: range, with: replacement))
                        let result = (original as NSString).replacingCharacters(in: range, with: replacement)
                        let reference = SourceLineMap(result)
                        XCTAssertEqual(map.starts, reference.starts, "\(original.debugDescription) \(range) \(replacement.debugDescription)")
                        for offset in 0...(result as NSString).length {
                            XCTAssertEqual(map.position(at: offset).line, reference.position(at: offset).line)
                            XCTAssertEqual(map.position(at: offset).column, reference.position(at: offset).column)
                        }
                    }
                }
            }
        }
    }

    func testRepeatedLargeSingleLineEditsAndInvalidRangeDoNotLoseOffsets() {
        var map = SourceLineMap(String(repeating: "A", count: 1024 * 1024))
        for _ in 0..<40 { XCTAssertTrue(map.replace(range: NSRange(location: 0, length: 1), with: "B")) }
        XCTAssertEqual(map.starts, [0])
        XCTAssertEqual(map.position(at: 1024 * 1024).column, 1024 * 1024 + 1)
        XCTAssertFalse(map.replace(range: NSRange(location: NSNotFound, length: Int.max), with: "\n"))
        XCTAssertEqual(map.starts, [0])
        XCTAssertTrue(map.replace(range: NSRange(location: 1024 * 1024, length: 0), with: "\n😀"))
        XCTAssertEqual(map.starts, [0, 1024 * 1024 + 1])
        XCTAssertEqual(map.position(at: 1024 * 1024 + 3).column, 3)
    }

    @MainActor func testNativeEditorLineIndexTracksUnicodeNewlinesAndUndoRedo() throws {
        _ = NSApplication.shared
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let root = parent.appendingPathComponent("line-index-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let original = "a\n😀z"
        try Data(original.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store)
        let coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        view.insertText("x\ny", replacementRange: NSRange(location: 0, length: 1))
        XCTAssertNil(store.failure)
        XCTAssertEqual(store.text, "x\ny\n😀z")
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        view.insertText("😃\n", replacementRange: (view.string as NSString).range(of: "😀"))
        XCTAssertEqual(store.text, "x\ny\n😃\nz")
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        store.undo(); editor.synchronize(scroll, coordinator: coordinator)
        XCTAssertEqual(store.text, "x\ny\n😀z")
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        store.redo(); editor.synchronize(scroll, coordinator: coordinator)
        XCTAssertEqual(store.text, "x\ny\n😃\nz")
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data(original.utf8))
    }

    @MainActor func testCRLFInsertionAlignsEditorCaretAndLineMapForTheNextEdit() throws {
        _ = NSApplication.shared
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let root = parent.appendingPathComponent("crlf-line-index-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let original = "a\n😀z"
        try Data(original.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        view.insertText("x\r\ny", replacementRange: NSRange(location: 0, length: 1))
        XCTAssertNil(store.failure)
        XCTAssertEqual(store.text, "x\ny\n😀z")
        XCTAssertEqual(Array(view.string.utf16), Array(store.text.utf16))
        XCTAssertEqual(view.selectedRange().location, 3)
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        view.insertText("q", replacementRange: (view.string as NSString).range(of: "😀"))
        XCTAssertNil(store.failure)
        XCTAssertEqual(store.text, "x\ny\nqz")
        XCTAssertEqual(Array(view.string.utf16), Array(store.text.utf16))
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        store.undo(); editor.synchronize(scroll, coordinator: coordinator)
        XCTAssertEqual(store.text, "x\ny\n😀z")
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data(original.utf8))
    }

    @MainActor func testRulerUsesTransientCompositionLinesBeforeOneNormalizedCommit() throws {
        _ = NSApplication.shared
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let root = parent.appendingPathComponent("marked-line-index-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let original = "a\n😀z"
        try Data(original.utf8).write(to: root.appendingPathComponent("main.tex"))
        let store = WorkspaceStore(); store.open(root)
        let editor = NativeEditor(store: store), coordinator = editor.makeCoordinator()
        let scroll = editor.makeScrollView(coordinator: coordinator)
        scroll.frame = NSRect(x: 0, y: 0, width: 500, height: 240)
        editor.synchronize(scroll, coordinator: coordinator)
        let view = try XCTUnwrap(scroll.documentView as? SourceTextView)
        let ruler = try XCTUnwrap(scroll.verticalRulerView as? LineNumberRuler)
        view.setMarkedText("x\r\n😀", selectedRange: NSRange(location: 5, length: 0), replacementRange: NSRange(location: 0, length: 1))
        XCTAssertTrue(store.isComposing)
        XCTAssertEqual(store.text, original)
        XCTAssertFalse(store.canUndo)
        XCTAssertEqual(ruler.displayedLineMap.starts, SourceLineMap(view.string).starts)
        let caret = SourceLineMap(view.string).position(at: view.selectedRange().location)
        XCTAssertEqual(store.caretLine, caret.line)
        XCTAssertEqual(store.caretColumn, caret.column)
        view.unmarkText()
        XCTAssertFalse(store.isComposing)
        XCTAssertNil(store.failure)
        XCTAssertEqual(store.text, "x\n😀\n😀z")
        XCTAssertEqual(Array(view.string.utf16), Array(store.text.utf16))
        XCTAssertEqual(coordinator.lineMap.starts, SourceLineMap(store.text).starts)
        store.undo(); editor.synchronize(scroll, coordinator: coordinator)
        XCTAssertEqual(store.text, original)
        XCTAssertFalse(store.canUndo)
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), Data(original.utf8))
    }
}
