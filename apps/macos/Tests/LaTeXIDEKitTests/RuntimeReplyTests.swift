import XCTest
@testable import LaTeXIDEKit

final class RuntimeReplyTests: XCTestCase {
    private let document = "\"id\":\"main.tex\",\"revision\":0,\"dirty\":false,\"last_origin\":\"user\",\"transaction_id\":0"
    private func decode(_ raw: String, _ op: String = "workspace_info") throws -> [String: Any] {
        try RuntimeReply.decode(Data(raw.utf8), operation: op)
    }
    private func chunk(_ text: String, _ offset: Int, _ next: Int, _ total: Int, _ revision: UInt64 = 0) -> [String: Any] {
        ["id":"main.tex", "revision":revision, "offset_bytes":offset, "next_offset":next, "total_bytes":total,
         "text":text, "done":next == total, "dirty":true, "bom":true, "line_ending":"crlf", "last_origin":"agent"]
    }
    func testMalformedEnvelopesAndWrongOperationAreErrors() {
        for raw in ["[]", "null", "{", "{\"ok\":1,\"result\":{}}", "{\"ok\":true,\"result\":[]}",
                    "{\"ok\":true,\"result\":{},\"error\":{\"code\":\"x\",\"message\":\"x\"}}",
                    "{\"ok\":false,\"error\":{\"code\":12,\"message\":\"x\"}}",
                    "{\"ok\":true,\"result\":{\(document)}}"] {
            XCTAssertThrowsError(try decode(raw), raw)
        }
        XCTAssertThrowsError(try decode("{\"ok\":true,\"version\":2,\"result\":{\(document)}}", "open_document"))
        XCTAssertThrowsError(try decode("{\"ok\":true,\"op\":\"undo\",\"result\":{\(document)}}", "open_document"))
        XCTAssertThrowsError(try decode(String(repeating: " ", count: RuntimeReply.frameLimit + 1)))
    }
    func testExactUInt64IncludesZeroAndMaximumAndRejectsMalformedFields() throws {
        for value in ["0", "9007199254740993", "18446744073709551615"] {
            let raw = "{\"ok\":true,\"result\":{\(document.replacingOccurrences(of: "\"revision\":0", with: "\"revision\":\(value)"))}}"
            XCTAssertEqual(try RuntimeReply.uint64(decode(raw, "open_document")["revision"]), UInt64(value))
        }
        for value in ["null", "true", "\"1\"", "-1", "1.5", "18446744073709551616"] {
            let raw = "{\"ok\":true,\"result\":{\(document.replacingOccurrences(of: "\"revision\":0", with: "\"revision\":\(value)"))}}"
            XCTAssertThrowsError(try decode(raw, "open_document"), value)
        }
        XCTAssertThrowsError(try decode("{\"ok\":true,\"result\":{\(document.replacingOccurrences(of: ",\"transaction_id\":0", with: ""))}}", "open_document"))
    }
    func testRawDuplicateKeysAndNegativeZeroAreRejectedBeforeFoundation() throws {
        let valid = #"{"ok":true,"result":{\#(document)}}"#
        for raw in [valid.replacingOccurrences(of: #""ok":true"#, with: #""ok":true,"ok":false"#),
                    valid.replacingOccurrences(of: #""revision":0"#, with: #""revision":1,"revision":2"#),
                    valid.replacingOccurrences(of: #""revision":0"#, with: #""revision":1,"revi\u0073ion":2"#),
                    valid.replacingOccurrences(of: #""revision":0"#, with: #""revision":-0"#),
                    valid.replacingOccurrences(of: #""transaction_id":0"#, with: #""transaction_id":-0"#)] {
            XCTAssertThrowsError(try decode(raw, "open_document")) { XCTAssertEqual(($0 as? RuntimeFailure)?.code, "invalid_reply") }
        }
        let escaped = valid.replacingOccurrences(of: #""revision":0"#, with: #""revi\u0073ion":18446744073709551615"#)
        XCTAssertEqual(try RuntimeReply.uint64(decode(escaped, "open_document")["revision"]), UInt64.max)
    }
    func testRemoteErrorKeepsItsMachineCode() {
        XCTAssertThrowsError(try decode("{\"ok\":false,\"error\":{\"code\":\"revision_conflict\",\"message\":\"changed\"}}")) {
            XCTAssertEqual(($0 as? RuntimeFailure)?.code, "revision_conflict")
        }
    }
    func testDecodedStringsPreserveLogicalFEFFAndExactChunkBytes() throws {
        for text in ["\u{feff}A😀", "X\u{feff}Y", "\u{feff}\u{feff}Z", "\u{feff}{\"revision\":-0}"] {
            let row = chunk(text, 0, text.utf8.count, text.utf8.count)
            let data = try JSONSerialization.data(withJSONObject: ["ok":true, "result":row])
            let decoded = try RuntimeReply.decode(data, operation: "read_document")
            XCTAssertEqual(Array(try RuntimeReply.string(decoded["text"]).utf8), Array(text.utf8))
            var wrongLength = row; wrongLength["next_offset"] = text.utf8.count - 1
            let invalid = try JSONSerialization.data(withJSONObject: ["ok":true, "result":wrongLength])
            XCTAssertThrowsError(try RuntimeReply.decode(invalid, operation: "read_document"))
        }
        let escaped = #"{"ok":true,"result":{"id":"main.tex","revision":0,"offset_bytes":0,"next_offset":4,"total_bytes":4,"text":"\uFEFFA","done":true,"dirty":false,"bom":false,"line_ending":"lf","last_origin":"user"}}"#
        XCTAssertEqual(Array(try RuntimeReply.string(decode(escaped, "read_document")["text"]).utf8), [239,187,191,65])
        let domain = #"{"ok":false,"error":{"code":"revision_conflict","message":"\uFEFFchanged"}}"#
        XCTAssertThrowsError(try decode(domain)) {
            XCTAssertEqual(($0 as? RuntimeFailure)?.code, "revision_conflict")
            XCTAssertEqual(Array((($0 as? RuntimeFailure)?.message ?? "").utf8), Array("\u{feff}changed".utf8))
        }
    }
    func testEveryCheckedUnsignedReplyFieldRejectsNegativeZero() throws {
        let vectors: [(String, String, [String])] = [
            ("read_document", #"{"id":"main.tex","revision":0,"offset_bytes":0,"next_offset":0,"total_bytes":0,"text":"","done":true,"dirty":false,"bom":false,"line_ending":"lf","last_origin":"user"}"#, ["revision","offset_bytes","next_offset","total_bytes"]),
            ("list_project_files", #"{"total":0,"offset":0,"files":[]}"#, ["total","offset"]),
            ("list_project_files", #"{"total":1,"offset":0,"files":[{"path":"main.tex","size":0}]}"#, ["size"]),
            ("get_syntax", #"{"document_id":"main.tex","revision":0,"offset_utf16":0,"length_utf16":0,"spans":[]}"#, ["offset_utf16","length_utf16"]),
            ("get_completions", #"{"document_id":"main.tex","revision":0,"prefix":"","replace_start_utf16":0,"replace_length_utf16":0,"items":[{"label":"x","detail":"x","insert_text":"x","selection_start_utf16":0,"selection_length_utf16":0}],"index_status":{"indexed_files":0,"skipped_files":0,"truncated":false}}"#, ["replace_start_utf16","replace_length_utf16","selection_start_utf16","selection_length_utf16","indexed_files","skipped_files"]),
            ("get_definition", #"{"document_id":"main.tex","revision":0,"symbol":null,"kind":null,"source_start_utf16":0,"source_length_utf16":0,"alternatives":0,"target":{"document_id":"main.tex","start_utf16":0,"length_utf16":0,"line":0,"revision":0}}"#, ["source_start_utf16","source_length_utf16","alternatives","start_utf16","length_utf16","line"]),
            ("get_diagnostics", #"{"build_id":null,"diagnostics":[{"severity":"warning","message":"x","file":null,"line":0}]}"#, ["line"])
        ]
        for (operation, result, fields) in vectors {
            let valid = "{\"ok\":true,\"result\":\(result)}"
            _ = try decode(valid, operation)
            for field in fields {
                let raw = valid.replacingOccurrences(of: "\"\(field)\":0", with: "\"\(field)\":-0")
                XCTAssertNotEqual(raw, valid)
                XCTAssertThrowsError(try decode(raw, operation), "\(operation).\(field)") {
                    XCTAssertEqual(($0 as? RuntimeFailure)?.code, "invalid_reply")
                }
            }
        }
        for index in 0..<3 {
            var span = ["0","0","0"]; span[index] = "-0"
            let raw = #"{"ok":true,"result":{"document_id":"main.tex","revision":0,"offset_utf16":0,"length_utf16":0,"spans":[[\#(span.joined(separator: ","))]]}}"#
            XCTAssertThrowsError(try decode(raw, "get_syntax"))
        }
        let fileMax = #"{"ok":true,"result":{"total":1,"offset":0,"files":[{"path":"main.tex","size":18446744073709551615}]}}"#
        let files = try RuntimeReply.objects(decode(fileMax, "list_project_files")["files"])
        XCTAssertEqual(try RuntimeReply.uint64(files[0]["size"]), UInt64.max)
    }
    private func completion(_ count: Int = 1, start: UInt64 = 4, length: UInt64 = 8) -> [String: Any] {
        let item: [String: Any] = ["label":"x", "detail":"x", "insert_text":"x", "selection_start_utf16":0, "selection_length_utf16":0]
        return ["document_id":"main.tex", "revision":UInt64(0), "prefix":"x",
                "replace_start_utf16":start, "replace_length_utf16":length,
                "items":Array(repeating:item, count:count)]
    }
    func testCompletionReplyRejectsMoreThanTwentyCandidates() throws {
        let arguments: [String: Any] = ["document_id":"main.tex", "expected_revision":UInt64(0), "caret_utf16":UInt64(8)]
        for count in [0, 1, 20] {
            let data = try JSONSerialization.data(withJSONObject: ["ok":true, "result":completion(count)])
            _ = try RuntimeReply.decode(data, operation:"get_completions", arguments:arguments)
        }
        let oversized = try JSONSerialization.data(withJSONObject: ["ok":true, "result":completion(21)])
        XCTAssertThrowsError(try RuntimeReply.decode(oversized, operation:"get_completions", arguments:arguments)) {
            XCTAssertEqual(($0 as? RuntimeFailure)?.code, "invalid_reply")
        }
    }
    func testCompletionReplacementContainsRequestedCaret() throws {
        let data = try JSONSerialization.data(withJSONObject: ["ok":true, "result":completion()])
        for caret in [UInt64(4), 8, 12] {
            _ = try RuntimeReply.decode(data, operation:"get_completions", arguments:["caret_utf16":caret])
        }
        for caret in [UInt64(0), 3, 13, UInt64.max] {
            XCTAssertThrowsError(try RuntimeReply.decode(data, operation:"get_completions", arguments:["caret_utf16":caret])) {
                XCTAssertEqual(($0 as? RuntimeFailure)?.code, "invalid_reply")
            }
        }
        let empty = try JSONSerialization.data(withJSONObject: ["ok":true, "result":completion(start:4, length:0)])
        _ = try RuntimeReply.decode(empty, operation:"get_completions", arguments:["caret_utf16":UInt64(4)])
        XCTAssertThrowsError(try RuntimeReply.decode(empty, operation:"get_completions", arguments:["caret_utf16":UInt64(5)]))
        let large = try JSONSerialization.data(withJSONObject: ["ok":true, "result":completion(start:UInt64(UInt32.max), length:UInt64(UInt32.max))])
        _ = try RuntimeReply.decode(large, operation:"get_completions", arguments:["caret_utf16":UInt64(UInt32.max)])
    }
    func testActualNativeABIPreservesLeadingAndChunkBoundaryLogicalFEFF() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let fixtures = ["logical.tex":"\u{feff}A😀\n", "boundary.tex":String(repeating:"A",count:65536) + "\u{feff}B\n"]
        // A physical encoding BOM is separate from the first logical scalar.
        for (name, text) in fixtures { try Data([239,187,191] + Array(text.utf8)).write(to:root.appendingPathComponent(name)) }
        let runtime = try NativeRuntime(root:root)
        for (name, text) in fixtures {
            _ = try runtime.call("open_document", ["path":name])
            let snapshot = try runtime.read(name)
            XCTAssertEqual(Array(snapshot.text.utf8), Array(text.utf8)); XCTAssertTrue(snapshot.bom)
            XCTAssertEqual(snapshot.lineEnding,"lf"); XCTAssertFalse(snapshot.dirty)
        }
    }
    func testChunkReaderPinsZeroRevisionAndPreservesUnicodeAndMetadata() throws {
        var calls = 0
        let snapshot = try NativeRuntime.readDocument("main.tex") { args in
            calls += 1
            if calls == 1 { XCTAssertNil(args["revision"]); return self.chunk("A😀", 0, 5, 7) }
            XCTAssertEqual(try RuntimeReply.uint64(args["revision"]), 0)
            XCTAssertEqual(try RuntimeReply.uint64(args["offset_bytes"]), 5)
            return self.chunk("\nB", 5, 7, 7)
        }
        XCTAssertEqual(calls, 2); XCTAssertEqual(snapshot.text, "A😀\nB")
        XCTAssertEqual(snapshot.revision, 0); XCTAssertTrue(snapshot.dirty); XCTAssertTrue(snapshot.bom)
        XCTAssertEqual(snapshot.lineEnding, "crlf")
    }
    func testChunkReaderRejectsNonprogressRevisionDriftAndWrongDocument() {
        let bad: [[String: Any]] = [
            ["revision":NSNull()], ["done":false, "next_offset":0, "text":""],
            ["id":"other.tex"], ["text":"x", "next_offset":5], ["total_bytes":1048577], ["done":1]
        ]
        for fields in bad {
            XCTAssertThrowsError(try NativeRuntime.readDocument("main.tex") { _ in self.chunk("A😀", 0, 5, 7).merging(fields) { _, new in new } })
        }
        for fields in [["revision":UInt64(1)], ["total_bytes":UInt64(8)]] {
            var calls = 0
            XCTAssertThrowsError(try NativeRuntime.readDocument("main.tex") { _ in
                calls += 1
                return calls == 1 ? self.chunk("A😀", 0, 5, 7) : self.chunk("\nB", 5, 7, 7).merging(fields) { _, new in new }
            })
            XCTAssertEqual(calls, 2)
        }
    }
    func testActualNativeABIReadsAndPreservesSavedUndo() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data([0xef, 0xbb, 0xbf] + Array("A😀\r\nB\r\n".utf8)).write(to: root.appendingPathComponent("main.tex"))
        let runtime = try NativeRuntime(root: root)
        XCTAssertThrowsError(try runtime.call("open_document", ["path":String(repeating: "x", count: RuntimeReply.frameLimit)])) {
            XCTAssertEqual(($0 as? RuntimeFailure)?.code, "request_too_large")
        }
        _ = try runtime.call("open_document", ["path":"main.tex"])
        XCTAssertThrowsError(try runtime.call("save_document", ["document_id":"main.tex", "expected_revision":UInt64.max])) {
            XCTAssertEqual(($0 as? RuntimeFailure)?.code, "revision_conflict", "The complete u64 must reach Rust without rounding")
        }
        let before = try runtime.read("main.tex")
        XCTAssertEqual(before.text, "A😀\nB\n"); XCTAssertTrue(before.bom)
        _ = try runtime.call("apply_edits", ["document_id":"main.tex", "expected_revision":before.revision,
            "edits":[["start":["line":0, "character":1], "end":["line":0, "character":3], "text":"C"]]])
        let edited = try runtime.read("main.tex")
        _ = try runtime.call("save_document", ["document_id":"main.tex", "expected_revision":edited.revision])
        _ = try runtime.call("undo")
        XCTAssertEqual(try runtime.read("main.tex").text, before.text)
        _ = try runtime.call("workspace_info")
        _ = try runtime.call("list_project_files")
    }
}
