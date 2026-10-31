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
}
