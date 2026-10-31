import Foundation

// Test-only, Foundation/Codable readers of the frozen Rust wire vectors.
// They do not register a production adapter or implement an IDE service.
enum NativeContractError: Error { case unsupportedVersion }
struct NativeVersion: Codable {
    let major: UInt32
    let minor: UInt32
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        major = try c.decode(UInt32.self, forKey: .major)
        minor = try c.decode(UInt32.self, forKey: .minor)
        guard major == 1 && minor == 0 else { throw NativeContractError.unsupportedVersion }
    }
}

@propertyWrapper struct NativeNullable<T: Codable>: Codable {
    var wrappedValue: T?
    init(wrappedValue: T?) { self.wrappedValue = wrappedValue }
    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        wrappedValue = c.decodeNil() ? nil : try c.decode(T.self)
    }
    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        if let value = wrappedValue { try c.encode(value) } else { try c.encodeNil() }
    }
}
extension KeyedDecodingContainer {
    func decode<T: Codable>(_ type: NativeNullable<T>.Type, forKey key: Key) throws -> NativeNullable<T> {
        try decodeIfPresent(type, forKey: key) ?? NativeNullable(wrappedValue: nil)
    }
}

struct NativeEnvelope<T: Codable>: Codable {
    let version: NativeVersion
    let package_id: String
    let request_id: String
    @NativeNullable var snapshot_id: String?
    let payload: T
}
struct NativeRange: Codable { let start: UInt32; let length: UInt32 }
struct NativeFile: Codable {
    let path: String
    @NativeNullable var revision: UInt64?
    let content_sha256: String
    let utf8: [UInt8]
    let dirty: Bool
}
struct NativeSnapshot: Codable { let id: String; let epoch: UInt64; let files: [NativeFile] }
struct NativeAnchor: Codable {
    let document_id: String
    @NativeNullable var revision: UInt64?
    let content_sha256: String
    let line: UInt32; let character: UInt32; let range: NativeRange
}
struct NativeEdit: Codable {
    let document_id: String; let expected_revision: UInt64
    let start_utf16: UInt32; let length_utf16: UInt32; let replacement: String
}

struct SearchQuery: Codable {
    let text: String; let regex: Bool; let case_sensitive: Bool
    @NativeNullable var cursor: String?
    let max_results: UInt32
}
struct SearchRequest: Codable { let snapshot: NativeSnapshot; let query: SearchQuery }
struct SearchHit: Codable { let anchor: NativeAnchor; let preview: String }
struct SearchPage: Codable {
    let snapshot_id: String; let hits: [SearchHit]
    @NativeNullable var next_cursor: String?
}
struct EditorAction: Codable {
    let document_id: String; let revision: UInt64; let selections: [NativeRange]; let command: String
}
struct EditorRequest: Codable { let snapshot: NativeSnapshot; let action: EditorAction }
struct EditorTransaction: Codable { let edits: [NativeEdit]; let selections: [NativeRange] }

struct SettingValue: Codable { let key: String; let value: String }
struct SettingsLayer: Codable { let schema: UInt32; let values: [SettingValue] }
struct SettingsRequest: Codable { let user: SettingsLayer; let project: SettingsLayer; let profile: SettingsLayer }
struct EffectiveSettings: Codable { let schema: UInt32; let values: [SettingValue]; let provenance: [String] }

struct PdfBundle: Codable { let generation: String; let snapshot_id: String; let pdf_path: String; let synctex_path: String }
struct SyncRequest: Codable { let bundle: PdfBundle; let source: NativeAnchor }
struct PdfLocation: Codable {
    let page: UInt32; let x: Double; let y: Double; let width: Double; let height: Double; let generation: String
}

struct AssistantState: Codable {
    let session_id: String; let draft: String
    @NativeNullable var active_turn: String?
    let last_sequence: UInt64
}
struct AssistantEventKind: Codable {
    let type: String
    @NativeNullable var value: String?
    enum CodingKeys: String, CodingKey { case type, value }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        type = try c.decode(String.self, forKey: .type)
        value = try c.decodeIfPresent(String.self, forKey: .value)
        let withValue = ["draft_changed", "turn_started"]
        let withoutValue = ["turn_stopped", "disconnected", "reconnected"]
        guard (withValue.contains(type) && value != nil) || (withoutValue.contains(type) && value == nil) else {
            throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "Unknown assistant event")
        }
    }
    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(type, forKey: .type)
        if let value { try c.encode(value, forKey: .value) }
    }
}
struct AssistantEvent: Codable { let session_id: String; let sequence: UInt64; let kind: AssistantEventKind }
struct AssistantRequest: Codable { let state: AssistantState; let event: AssistantEvent }
