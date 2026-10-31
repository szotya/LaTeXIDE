import Foundation

struct DefinitionTarget: Decodable {
    let documentID: String
    let revision: UInt64?
    let startUTF16: Int, lengthUTF16: Int, line: Int
    var range: NSRange { NSRange(location: startUTF16, length: lengthUTF16) }
    enum CodingKeys: String, CodingKey {
        case documentID = "document_id", revision, startUTF16 = "start_utf16", lengthUTF16 = "length_utf16", line
    }
}
struct DefinitionReply: Decodable {
    let documentID: String, revision: UInt64
    let symbol: String?
    let target: DefinitionTarget?
    let alternatives: Int
    let indexStatus: CompletionIndexStatus?
    enum CodingKeys: String, CodingKey {
        case documentID = "document_id", revision, symbol, target, alternatives, indexStatus = "index_status"
    }
}

/// Resolve/read/re-resolve before the store changes visible tabs or selection.
enum DefinitionResolver {
    static func resolve(runtime: NativeRuntime, source: String, revision: UInt64, offset: Int) throws -> DefinitionReply {
        func query() throws -> DefinitionReply {
            let result = try runtime.call("get_definition", ["document_id": source, "expected_revision": revision, "offset_utf16": offset])
            let reply = try JSONDecoder().decode(DefinitionReply.self, from: JSONSerialization.data(withJSONObject: result))
            guard reply.documentID == source, reply.revision == revision else { throw RuntimeFailure(message: "A forrás közben megváltozott. Kattints újra a definícióhoz.") }
            return reply
        }
        let initial = try query()
        guard let original = initial.target else { return initial }
        let document = try runtime.read(original.documentID)
        let fresh = try query()
        guard let target = fresh.target, target.documentID == original.documentID,
              target.revision == document.revision, let symbol = fresh.symbol,
              target.lengthUTF16 > 0,
              (try? SourceEdit.make(text: document.text, range: target.range, replacement: "")) != nil,
              (document.text as NSString).substring(with: target.range) == symbol else {
            throw RuntimeFailure(message: "A definíció közben megváltozott. Kattints újra a friss helyhez.")
        }
        return fresh
    }
}
