import Foundation
import CoreFoundation
import CLatexCore

struct RuntimeFailure: LocalizedError {
    let message: String
    var code: String = "invalid_reply"
    var errorDescription: String? { message }
}

// Checked legacy v1 replies. This is an adapter guard, not a generated full
// schema or wire request/operation ID negotiation.
enum RuntimeReply {
    static let frameLimit = 1024 * 1024
    static func invalid() -> RuntimeFailure { RuntimeFailure(message: "Hibás vagy nem kompatibilis runtime-válasz.") }
    static func object(_ value: Any?) throws -> [String: Any] {
        guard let result = value as? [String: Any] else { throw invalid() }; return result
    }
    static func string(_ value: Any?) throws -> String {
        guard let result = value as? String else { throw invalid() }; return result
    }
    static func boolean(_ value: Any?) throws -> Bool {
        guard let number = value as? NSNumber, CFGetTypeID(number) == CFBooleanGetTypeID() else { throw invalid() }
        return number.boolValue
    }
    static func uint64(_ value: Any?) throws -> UInt64 {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
              !["f", "d"].contains(String(cString: number.objCType)),
              let result = UInt64(number.stringValue) else { throw invalid() }
        return result
    }
    static func bounded(_ value: Any?, _ max: UInt64 = UInt64(UInt32.max)) throws -> UInt64 {
        let result = try uint64(value); guard result <= max else { throw invalid() }; return result
    }
    static func objects(_ value: Any?) throws -> [[String: Any]] {
        guard let result = value as? [[String: Any]] else { throw invalid() }; return result
    }
    static func optionalString(_ value: Any?) throws {
        if let value, !(value is NSNull) { _ = try string(value) }
    }
    static func document(_ value: [String: Any]) throws {
        _ = try string(value["id"]); _ = try uint64(value["revision"])
        _ = try boolean(value["dirty"]); _ = try string(value["last_origin"])
        _ = try uint64(value["transaction_id"])
    }
    static func diagnostics(_ value: Any?) throws {
        for row in try objects(value) {
            _ = try string(row["severity"]); _ = try string(row["message"])
            try optionalString(row["file"])
            if let line = row["line"], !(line is NSNull) { _ = try bounded(line) }
        }
    }
    static func build(_ value: [String: Any]) throws {
        _ = try string(value["id"])
        let state = try string(value["state"])
        guard ["queued", "running", "succeeded", "failed", "cancelled", "timed_out"].contains(state) else { throw invalid() }
        try optionalString(value["pdf_path"])
    }
    static func index(_ value: Any?) throws {
        guard let value, !(value is NSNull) else { return }
        let row = try object(value)
        _ = try bounded(row["indexed_files"]); _ = try bounded(row["skipped_files"])
        _ = try boolean(row["truncated"])
    }
    // Decode strings without JSONSerialization's loss of a leading logical
    // U+FEFF. Keep Foundation's exact numeric representation, but preserve -0's
    // sign so every existing uint64/bounded check rejects it, including arrays.
    private struct RawJSON {
        let bytes: [UInt8]
        var cursor = 0
        mutating func space() { while cursor < bytes.count && [9, 10, 13, 32].contains(bytes[cursor]) { cursor += 1 } }
        mutating func take(_ byte: UInt8) -> Bool {
            space()
            if cursor < bytes.count && bytes[cursor] == byte { cursor += 1; return true }
            return false
        }
        func digit(_ byte: UInt8) -> Bool { byte >= 48 && byte <= 57 }
        mutating func stringToken() throws -> Range<Int> {
            space(); let start = cursor
            guard take(34) else { throw RuntimeReply.invalid() }
            while cursor < bytes.count {
                let byte = bytes[cursor]; cursor += 1
                if byte == 34 { return start..<cursor }
                guard byte >= 32 else { throw RuntimeReply.invalid() }
                if byte == 92 {
                    guard cursor < bytes.count else { throw RuntimeReply.invalid() }
                    let escape = bytes[cursor]; cursor += 1
                    if escape == 117 {
                        guard cursor + 4 <= bytes.count else { throw RuntimeReply.invalid() }
                        for byte in bytes[cursor..<(cursor + 4)] {
                            guard digit(byte) || (65...70).contains(byte) || (97...102).contains(byte) else { throw RuntimeReply.invalid() }
                        }
                        cursor += 4
                    } else if ![34, 92, 47, 98, 102, 110, 114, 116].contains(escape) { throw RuntimeReply.invalid() }
                }
            }
            throw RuntimeReply.invalid()
        }
        mutating func number(_ key: String?) throws -> Any {
            let start = cursor
            if bytes[cursor] == 45 { cursor += 1 }
            guard cursor < bytes.count else { throw RuntimeReply.invalid() }
            if bytes[cursor] == 48 { cursor += 1 }
            else {
                guard (49...57).contains(bytes[cursor]) else { throw RuntimeReply.invalid() }
                while cursor < bytes.count && digit(bytes[cursor]) { cursor += 1 }
            }
            if cursor < bytes.count && bytes[cursor] == 46 {
                cursor += 1; let first = cursor
                while cursor < bytes.count && digit(bytes[cursor]) { cursor += 1 }
                guard first < cursor else { throw RuntimeReply.invalid() }
            }
            if cursor < bytes.count && [69, 101].contains(bytes[cursor]) {
                cursor += 1
                if cursor < bytes.count && [43, 45].contains(bytes[cursor]) { cursor += 1 }
                let first = cursor
                while cursor < bytes.count && digit(bytes[cursor]) { cursor += 1 }
                guard first < cursor else { throw RuntimeReply.invalid() }
            }
            if key == "revision" || key == "transaction_id" {
                let token = bytes[start..<cursor]
                guard token.allSatisfy({ digit($0) }), UInt64(String(decoding: token, as: UTF8.self)) != nil else { throw RuntimeReply.invalid() }
            }
            let token = bytes[start..<cursor]
            if token.count == 2, token.first == 45, token.last == 48 { return NSNumber(value: -0.0) }
            return try JSONSerialization.jsonObject(with: Data(token), options: [.fragmentsAllowed])
        }
        mutating func value(_ key: String? = nil, depth: Int = 0) throws -> Any {
            space()
            guard depth <= 64, cursor < bytes.count else { throw RuntimeReply.invalid() }
            switch bytes[cursor] {
            case 123:
                cursor += 1; var keys = Set<String>(), result: [String: Any] = [:]
                if take(125) { return result }
                repeat {
                    let range = try stringToken()
                    let name = try JSONDecoder().decode(String.self, from: Data(bytes[range]))
                    guard keys.insert(name).inserted, take(58) else { throw RuntimeReply.invalid() }
                    result[name] = try value(name, depth: depth + 1)
                } while take(44)
                guard take(125) else { throw RuntimeReply.invalid() }
                return result
            case 91:
                cursor += 1; var result: [Any] = []
                if take(93) { return result }
                repeat { result.append(try value(depth: depth + 1)) } while take(44)
                guard take(93) else { throw RuntimeReply.invalid() }
                return result
            case 34:
                let range = try stringToken()
                return try JSONDecoder().decode(String.self, from: Data(bytes[range]))
            case 45, 48...57: return try number(key)
            default:
                for word in ["true", "false", "null"] {
                    let literal = Array(word.utf8)
                    if bytes[cursor...].starts(with: literal) {
                        cursor += literal.count
                        if word == "null" { return NSNull() }
                        return NSNumber(value: word == "true")
                    }
                }
                throw RuntimeReply.invalid()
            }
        }
        mutating func check() throws -> Any {
            let result = try value(); space()
            guard cursor == bytes.count else { throw RuntimeReply.invalid() }
            return result
        }
    }

    static func decode(_ data: Data, operation: String, arguments: [String: Any] = [:]) throws -> [String: Any] {
        guard data.count <= frameLimit else { throw invalid() }
        let reply: [String: Any]
        do {
            var raw = RawJSON(bytes: Array(data))
            reply = try object(raw.check())
        }
        catch { throw invalid() }
        let ok = try boolean(reply["ok"])
        if let version = reply["version"] { guard try uint64(version) == 1 else { throw invalid() } }
        if let op = reply["op"] { guard try string(op) == operation else { throw invalid() } }
        if !ok {
            guard reply["result"] == nil || reply["result"] is NSNull else { throw invalid() }
            let error = try object(reply["error"])
            let code = try string(error["code"]), message = try string(error["message"])
            guard !code.isEmpty else { throw invalid() }
            throw RuntimeFailure(message: message, code: code)
        }
        guard reply["error"] == nil || reply["error"] is NSNull else { throw invalid() }
        let result = try object(reply["result"])
        try validate(result, operation: operation, arguments: arguments)
        return result
    }
    static func validate(_ row: [String: Any], operation: String, arguments: [String: Any]) throws {
        if let version = row["protocol_version"] { guard try uint64(version) == 1 else { throw invalid() } }
        switch operation {
        case "workspace_info":
            guard try uint64(row["protocol_version"]) == 1 else { throw invalid() }
            _ = try string(row["root"]); _ = try string(row["session_descriptor"])
            for name in ["can_undo", "can_redo", "compiler_ready"] { _ = try boolean(row[name]) }
            for item in try objects(row["open_documents"]) { try document(item) }
            if let value = row["build"], !(value is NSNull) { try build(object(value)) }
        case "open_document", "apply_edits", "end_composition", "save_document", "undo", "redo":
            try document(row)
        case "read_document":
            let id = try string(row["id"]), revision = try uint64(row["revision"])
            let offset = try bounded(row["offset_bytes"], UInt64(frameLimit))
            let next = try bounded(row["next_offset"], UInt64(frameLimit))
            let total = try bounded(row["total_bytes"], UInt64(frameLimit))
            let text = try string(row["text"]), done = try boolean(row["done"])
            _ = try boolean(row["dirty"]); _ = try boolean(row["bom"]); _ = try string(row["last_origin"])
            guard ["lf", "crlf"].contains(try string(row["line_ending"])),
                  offset <= next, next <= total, next - offset == text.utf8.count,
                  text.utf8.count <= 65536, done == (next == total), done || next > offset else { throw invalid() }
            if let expected = arguments["document_id"] { guard try string(expected) == id else { throw invalid() } }
            if let expected = arguments["offset_bytes"] { guard try uint64(expected) == offset else { throw invalid() } }
            if let expected = arguments["revision"] { guard try uint64(expected) == revision else { throw invalid() } }
        case "list_project_files":
            let total = try bounded(row["total"], UInt64(Int.max)), offset = try bounded(row["offset"], UInt64(Int.max))
            let files = try objects(row["files"])
            guard files.count <= 256, UInt64(files.count) <= total, offset <= UInt64(Int.max) else { throw invalid() }
            for item in files { _ = try string(item["path"]); _ = try uint64(item["size"]) }
        case "get_syntax", "get_completions", "get_definition":
            let id = try string(row["document_id"]), revision = try uint64(row["revision"])
            if let expected = arguments["document_id"] { guard try string(expected) == id else { throw invalid() } }
            if let expected = arguments["expected_revision"] { guard try uint64(expected) == revision else { throw invalid() } }
            if operation == "get_syntax" {
                _ = try bounded(row["offset_utf16"]); _ = try bounded(row["length_utf16"], 32768)
                guard let spans = row["spans"] as? [[Any]] else { throw invalid() }
                for span in spans { guard span.count == 3 else { throw invalid() }; for value in span { _ = try bounded(value) } }
            } else if operation == "get_completions" {
                _ = try string(row["prefix"])
                _ = try bounded(row["replace_start_utf16"]); _ = try bounded(row["replace_length_utf16"])
                for item in try objects(row["items"]) {
                    for key in ["label", "detail", "insert_text"] { _ = try string(item[key]) }
                    for key in ["selection_start_utf16", "selection_length_utf16"] { _ = try bounded(item[key]) }
                }
                try index(row["index_status"])
            } else {
                try optionalString(row["symbol"]); try optionalString(row["kind"])
                _ = try bounded(row["source_start_utf16"]); _ = try bounded(row["source_length_utf16"])
                _ = try bounded(row["alternatives"]); try index(row["index_status"])
                if let value = row["target"], !(value is NSNull) {
                    let target = try object(value); _ = try string(target["document_id"])
                    for key in ["start_utf16", "length_utf16", "line"] { _ = try bounded(target[key]) }
                    if let rev = target["revision"], !(rev is NSNull) { _ = try uint64(rev) }
                }
            }
        case "begin_composition": guard try boolean(row["active"]) else { throw invalid() }
        case "configure_compiler": guard try boolean(row["ready"]) else { throw invalid() }
        case "request_build": _ = try string(row["id"])
        case "cancel_build": _ = try string(row["id"]); guard try boolean(row["cancel_requested"]) else { throw invalid() }
        case "get_diagnostics": try optionalString(row["build_id"]); try diagnostics(row["diagnostics"])
        case "get_operation":
            try build(row); _ = try string(row["log"]); _ = try string(row["log_path"])
            _ = try boolean(row["log_truncated"]); _ = try boolean(row["stale"]); try diagnostics(row["diagnostics"])
        default: throw invalid()
        }
    }
}

final class NativeRuntime {
    private var handle: OpaquePointer
    init(root: URL) throws {
        guard let created = root.path.withCString({ latexide_runtime_new($0) }) else {
            throw RuntimeFailure(message: "A projekt nem nyitható meg (útvonal vagy IPC-jogosultság).", code: "invalid_project")
        }
        handle = created
    }
    deinit { latexide_runtime_free(handle) }
    func call(_ op: String, _ arguments: [String: Any] = [:]) throws -> [String: Any] {
        var request = arguments; request["op"] = op
        let data = try JSONSerialization.data(withJSONObject: request)
        guard data.count <= RuntimeReply.frameLimit else { throw RuntimeFailure(message: "A kérés meghaladja az 1 MiB határt.", code: "request_too_large") }
        let json = String(decoding: data, as: UTF8.self)
        guard let pointer = json.withCString({ latexide_dispatch(handle, $0) }) else { throw RuntimeReply.invalid() }
        defer { latexide_string_free(pointer) }
        let length = strnlen(pointer, RuntimeReply.frameLimit + 1)
        guard length <= RuntimeReply.frameLimit else { throw RuntimeReply.invalid() }
        return try RuntimeReply.decode(Data(bytes: pointer, count: length), operation: op, arguments: arguments)
    }
    func read(_ id: String) throws -> (text: String, revision: UInt64, dirty: Bool, lineEnding: String, bom: Bool) {
        try Self.readDocument(id, call: { try self.call("read_document", $0) })
    }
    static func readDocument(_ id: String, call: ([String: Any]) throws -> [String: Any]) throws
        -> (text: String, revision: UInt64, dirty: Bool, lineEnding: String, bom: Bool) {
        var offset: UInt64 = 0, revision: UInt64?, total: UInt64?, text = ""
        while true {
            var parameters: [String: Any] = ["document_id": id, "offset_bytes": offset]
            if let revision { parameters["revision"] = revision }
            let part = try call(parameters)
            try RuntimeReply.validate(part, operation: "read_document", arguments: parameters)
            let nextTotal = try RuntimeReply.uint64(part["total_bytes"])
            if let total { guard total == nextTotal else { throw RuntimeReply.invalid() } }
            total = nextTotal; revision = try RuntimeReply.uint64(part["revision"])
            text += try RuntimeReply.string(part["text"])
            if try RuntimeReply.boolean(part["done"]), let revision {
                return (text, revision, try RuntimeReply.boolean(part["dirty"]),
                        try RuntimeReply.string(part["line_ending"]), try RuntimeReply.boolean(part["bom"]))
            }
            offset = try RuntimeReply.uint64(part["next_offset"])
        }
    }
}
