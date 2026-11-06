import XCTest
import AppKit
@testable import LaTeXIDEKit

final class SourceLineMapTests: XCTestCase {
    func testIncrementalEditsMatchFreshLineMapsAtUTF16Boundaries() {
        for original in ["", "a\nb\n", "😀e\u{301}\nα\nend", "\n\n"] {
            for replacement in ["", "x", "😀", "\n", "p\nq\n"] {
                let units = Array(original.utf16)
                let boundaries = (0...units.count).filter { offset in
                    offset == 0 || offset == units.count ||
                    !(0xD800...0xDBFF).contains(units[offset - 1]) ||
                    !(0xDC00...0xDFFF).contains(units[offset])
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
}
