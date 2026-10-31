import Foundation

struct SourcePosition: Codable, Equatable {
    let line: Int
    let character: Int
}
struct SourceEdit: Encodable {
    let start: SourcePosition
    let end: SourcePosition
    let text: String
    static func make(text: String, range: NSRange, replacement: String) throws -> SourceEdit {
        let units = Array(text.utf16)
        func scalarBoundary(_ offset: Int) -> Bool {
            guard offset >= 0, offset <= units.count else { return false }
            return offset == 0 || offset == units.count || !(0xD800...0xDBFF).contains(units[offset - 1]) || !(0xDC00...0xDFFF).contains(units[offset])
        }
        guard range.location != NSNotFound, range.length <= units.count,
              range.location <= units.count - range.length,
              scalarBoundary(range.location), scalarBoundary(range.location + range.length),
              let swiftRange = Range(range, in: text) else {
            throw RuntimeFailure(message: "Érvénytelen UTF-16 tartomány.")
        }
        func position(_ index: String.Index) -> SourcePosition {
            let prefix = text[..<index]
            let lines = prefix.split(separator: "\n", omittingEmptySubsequences: false)
            return SourcePosition(line: lines.count - 1, character: lines.last!.utf16.count)
        }
        return SourceEdit(start: position(swiftRange.lowerBound), end: position(swiftRange.upperBound), text: replacement)
    }
    // Used only for native marked-text commits, which can involve several transient changes.
    static func difference(from before: String, to after: String) -> (range: NSRange, replacement: String) {
        let lhs = Array(before.unicodeScalars), rhs = Array(after.unicodeScalars)
        var prefix = 0, suffix = 0
        while prefix < min(lhs.count, rhs.count), lhs[prefix] == rhs[prefix] { prefix += 1 }
        while suffix < min(lhs.count, rhs.count) - prefix, lhs[lhs.count - 1 - suffix] == rhs[rhs.count - 1 - suffix] { suffix += 1 }
        func string(_ scalars: ArraySlice<Unicode.Scalar>) -> String { String(String.UnicodeScalarView(scalars)) }
        let start = string(lhs[..<prefix]).utf16.count
        let removed = string(lhs[prefix..<(lhs.count - suffix)]).utf16.count
        return (NSRange(location: start, length: removed), string(rhs[prefix..<(rhs.count - suffix)]))
    }
}
