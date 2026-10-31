import Foundation
import CLatexCore

struct RuntimeFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
final class NativeRuntime {
    private var handle: OpaquePointer
    init(root: URL) throws {
        guard let created = root.path.withCString({ latexide_runtime_new($0) }) else {
            throw RuntimeFailure(message: "A projekt nem nyitható meg (útvonal vagy IPC-jogosultság).")
        }
        handle = created
    }
    deinit { latexide_runtime_free(handle) }
    func call(_ op: String, _ arguments: [String: Any] = [:]) throws -> [String: Any] {
        var request = arguments
        request["op"] = op
        let data = try JSONSerialization.data(withJSONObject: request)
        let json = String(decoding: data, as: UTF8.self)
        guard let pointer = json.withCString({ latexide_dispatch(handle, $0) }) else { throw RuntimeFailure(message: "Üres runtime-válasz.") }
        defer { latexide_string_free(pointer) }
        let reply = try JSONSerialization.jsonObject(with: Data(String(cString: pointer).utf8)) as! [String: Any]
        guard reply["ok"] as? Bool == true else {
            let error = reply["error"] as? [String: Any]
            throw RuntimeFailure(message: error?["message"] as? String ?? "Ismeretlen runtime-hiba.")
        }
        return reply["result"] as? [String: Any] ?? [:]
    }
    func read(_ id: String) throws -> (text: String, revision: UInt64, dirty: Bool, lineEnding: String, bom: Bool) {
        var offset = 0, revision: UInt64?, text = "", dirty = false
        var lineEnding = "lf", bom = false
        while true {
            var parameters: [String: Any] = ["document_id": id, "offset_bytes": offset]
            if let revision { parameters["revision"] = revision }
            let part = try call("read_document", parameters)
            text += part["text"] as! String
            revision = (part["revision"] as! NSNumber).uint64Value
            dirty = part["dirty"] as? Bool ?? false
            lineEnding = part["line_ending"] as? String ?? "lf"; bom = part["bom"] as? Bool ?? false
            if part["done"] as? Bool == true { return (text, revision!, dirty, lineEnding, bom) }
            offset = part["next_offset"] as! Int
        }
    }
}
