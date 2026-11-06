import XCTest
import Foundation
import Dispatch
@testable import LaTeXIDEKit

final class RecoveryLatencyTests: XCTestCase {
    func testNativeDurableEditLatencyAndRecoveredUndo() throws {
        let manager = FileManager.default
        let parent = ProcessInfo.processInfo.environment["PR68_NATIVE_SCRATCH"].map { URL(fileURLWithPath: $0) } ?? manager.temporaryDirectory
        let base = parent.appendingPathComponent("recovery-latency-" + UUID().uuidString)
        try manager.createDirectory(at: base, withIntermediateDirectories: true)
        for size in [1024, 1024 * 1024] {
            let root = base.appendingPathComponent(String(size))
            try manager.createDirectory(at: root, withIntermediateDirectories: true)
            let original = Data(repeating: 65, count: size)
            try original.write(to: root.appendingPathComponent("main.tex"))
            var samples: [UInt64] = []
            var revision: UInt64 = 0
            do {
                let runtime = try NativeRuntime(root: root)
                let status = try runtime.call("workspace_info")["recovery"] as? [String: Any]
                XCTAssertEqual(status?["enabled"] as? Bool, true)
                revision = try RuntimeReply.uint64(runtime.call("open_document", ["path": "main.tex"])["revision"])
                for index in 0..<40 {
                    let start = DispatchTime.now().uptimeNanoseconds
                    let reply = try runtime.call("apply_edits", ["document_id": "main.tex", "expected_revision": revision,
                        "edits": [["start": ["line": 0, "character": 0], "end": ["line": 0, "character": 1],
                                   "text": index % 2 == 0 ? "B" : "C"]]])
                    samples.append(DispatchTime.now().uptimeNanoseconds - start)
                    revision = try RuntimeReply.uint64(reply["revision"])
                }
                XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), original)
            }
            let runtime = try NativeRuntime(root: root)
            let restored = try runtime.read("main.tex")
            XCTAssertEqual(restored.revision, revision)
            XCTAssertTrue(restored.text.hasPrefix("C"))
            XCTAssertTrue(restored.dirty)
            _ = try runtime.call("undo")
            XCTAssertTrue(try runtime.read("main.tex").text.hasPrefix("B"))
            XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("main.tex")), original)
            let sorted = samples.sorted()
            let result: [String: Any] = ["scope": "native adapter/public C ABI/runtime lock; UI event latency unmeasured",
                "bytes": size, "samples": samples, "p50_ns": sorted[19], "p95_ns": sorted[37],
                "p99_ns": sorted[39], "max_ns": sorted[39], "fixture": root.path]
            let data = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
            print("PR68_NATIVE_LATENCY " + String(decoding: data, as: UTF8.self))
            try data.write(to: root.appendingPathComponent("latency.json"), options: [.atomic])
        }
    }
}
