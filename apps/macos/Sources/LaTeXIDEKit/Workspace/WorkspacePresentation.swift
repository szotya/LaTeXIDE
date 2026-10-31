import Foundation

struct EditorTab: Identifiable, Equatable {
    let id: String
    var revision: UInt64
    var dirty: Bool
    var origin: String
    var title: String { (id as NSString).lastPathComponent }
    init(id: String, revision: UInt64, dirty: Bool, origin: String = "user") { self.id = id; self.revision = revision; self.dirty = dirty; self.origin = origin }
    init?(_ data: [String: Any]) {
        guard let id = data["id"] as? String, let revision = data["revision"] as? NSNumber, let dirty = data["dirty"] as? Bool else { return nil }
        self.init(id: id, revision: revision.uint64Value, dirty: dirty, origin: data["last_origin"] as? String ?? "disk")
    }
}
enum SidebarTool: String { case project, files, agent }
enum BottomTool: String { case problems, output }
enum QuickOpenMode: String { case files, commands }
enum InspectorTab: String { case pdf, assistant }
struct EditorViewState {
    var selection: NSRange
    var scrollOrigin: CGPoint
    static func safeSelection(_ range: NSRange, in source: String) -> NSRange {
        let text = source as NSString, count = text.length
        func splitsScalar(_ index: Int) -> Bool {
            index > 0 && index < count && (0xD800...0xDBFF).contains(text.character(at: index - 1)) && (0xDC00...0xDFFF).contains(text.character(at: index))
        }
        var start = min(max(0, range.location), count)
        var end = start + min(max(0, range.length), count - start)
        if splitsScalar(start) { start -= 1 }
        if range.length == 0 { end = start }
        else if splitsScalar(end) { end += 1 }
        return NSRange(location: start, length: end - start)
    }
}
struct EditorNavigation: Equatable {
    let serial: Int; let document: String; let line: Int
    var range: NSRange? = nil
    var revision: UInt64? = nil
}
struct CodeSpan: Equatable { let start: Int; let length: Int; let kind: Int }
struct EditorCompletionItem: Decodable, Identifiable {
    let kind: String?
    let label: String, detail: String, insertText: String
    let selectionStartUTF16: Int, selectionLengthUTF16: Int
    var id: String { label }
    enum CodingKeys: String, CodingKey {
        case kind, label, detail, insertText = "insert_text", selectionStartUTF16 = "selection_start_utf16", selectionLengthUTF16 = "selection_length_utf16"
    }
}
struct CompletionIndexStatus: Decodable, Equatable {
    let indexedFiles: Int, truncated: Bool, skippedFiles: Int
    enum CodingKeys: String, CodingKey { case indexedFiles = "indexed_files", truncated, skippedFiles = "skipped_files" }
}
struct CompletionProposal: Decodable {
    let documentID: String
    let revision: UInt64
    let replaceStartUTF16: Int, replaceLengthUTF16: Int
    let prefix: String
    let items: [EditorCompletionItem]
    let indexStatus: CompletionIndexStatus?
    var range: NSRange { NSRange(location: replaceStartUTF16, length: replaceLengthUTF16) }
    enum CodingKeys: String, CodingKey {
        case documentID = "document_id", revision, prefix, items, indexStatus = "index_status"
        case replaceStartUTF16 = "replace_start_utf16", replaceLengthUTF16 = "replace_length_utf16"
    }
}
struct CompilerDiagnostic: Identifiable, Equatable {
    let id: String, severity: String, message: String
    let file: String?
    let line: Int?
    init(index: Int, data: [String: Any]) {
        id = String(index); severity = data["severity"] as? String ?? "error"
        message = data["message"] as? String ?? ""; file = data["file"] as? String; line = data["line"] as? Int
    }
}
struct ProjectNode: Identifiable {
    let id: String, name: String
    let file: String?
    let children: [ProjectNode]?
    static func tree(files: [String]) -> [ProjectNode] {
        func build(_ paths: [String], prefix: String) -> [ProjectNode] {
            let grouped = Dictionary(grouping: paths) { path in String(path.dropFirst(prefix.count).split(separator: "/", maxSplits: 1).first ?? "") }
            return grouped.keys.sorted { lhs, rhs in
                let ldir = grouped[lhs]!.contains { $0.dropFirst(prefix.count + lhs.count).hasPrefix("/") }
                let rdir = grouped[rhs]!.contains { $0.dropFirst(prefix.count + rhs.count).hasPrefix("/") }
                return ldir != rdir ? ldir : lhs.localizedStandardCompare(rhs) == .orderedAscending
            }.map { name in
                let id = prefix + name, paths = grouped[name]!
                let folder = paths.contains { $0.dropFirst(id.count).hasPrefix("/") }
                return ProjectNode(id: id, name: name, file: folder ? nil : id, children: folder ? build(paths, prefix: id + "/") : nil)
            }
        }
        return build(files, prefix: "")
    }
}
