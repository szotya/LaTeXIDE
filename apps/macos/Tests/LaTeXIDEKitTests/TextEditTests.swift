import XCTest
@testable import LaTeXIDEKit

final class TextEditTests: XCTestCase {
    func testUTF16RangeAfterEmojiAndCombiningMarks() throws {
        let text = "első\n😀e\u{301} vége"
        let edit = try SourceEdit.make(text: text, range: NSRange(location: 9, length: 1), replacement: "!")
        XCTAssertEqual(edit.start.line, 1)
        XCTAssertEqual(edit.start.character, 4)
        XCTAssertEqual(edit.end.character, 5)
    }
    func testCompositionDeltaDoesNotSplitEmoji() throws {
        let delta = SourceEdit.difference(from: "a😀z", to: "a😃z")
        XCTAssertEqual(delta.range, NSRange(location: 1, length: 2))
        XCTAssertEqual(delta.replacement, "😃")
    }
    func testRejectsSurrogateSplit() {
        XCTAssertThrowsError(try SourceEdit.make(text: "😀", range: NSRange(location: 1, length: 0), replacement: "x"))
    }
    func testScalarBoundariesKeepLiteralUTF16LineColumns() throws {
        let text = "a\n😀e\u{301}\nz"
        let cases: [(NSRange, Int, Int, Int, Int)] = [
            (NSRange(location: 2, length: 2), 1, 0, 1, 2),
            (NSRange(location: 4, length: 2), 1, 2, 1, 4),
            (NSRange(location: 5, length: 1), 1, 3, 1, 4),
            (NSRange(location: 7, length: 1), 2, 0, 2, 1),
            (NSRange(location: 8, length: 0), 2, 1, 2, 1)
        ]
        for (range, line, column, endLine, endColumn) in cases {
            let edit = try SourceEdit.make(text: text, range: range, replacement: "x")
            XCTAssertEqual(edit.start, SourcePosition(line: line, character: column))
            XCTAssertEqual(edit.end, SourcePosition(line: endLine, character: endColumn))
        }
        for range in [NSRange(location: 3, length: 0), NSRange(location: NSNotFound, length: 0), NSRange(location: 0, length: Int.max)] {
            XCTAssertThrowsError(try SourceEdit.make(text: text, range: range, replacement: "x"))
        }
    }
    func testLargeASCIIFrontAndFinalRangesStayExact() throws {
        let text = String(repeating: "A", count: 1024 * 1024)
        let front = try SourceEdit.make(text: text, range: NSRange(location: 0, length: 1), replacement: "B")
        XCTAssertEqual(front.start, SourcePosition(line: 0, character: 0))
        XCTAssertEqual(front.end, SourcePosition(line: 0, character: 1))
        let end = try SourceEdit.make(text: text, range: NSRange(location: 1_048_575, length: 1), replacement: "B")
        XCTAssertEqual(end.start, SourcePosition(line: 0, character: 1_048_575))
        XCTAssertEqual(end.end, SourcePosition(line: 0, character: 1_048_576))
    }
}
