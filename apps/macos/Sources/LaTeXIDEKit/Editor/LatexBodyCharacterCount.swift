import Foundation

enum EditorCharacterCountMode: String, CaseIterable {
    case source, latexBody
    static let preferenceKey = "ide.characterCountMode"
    static func restored(_ value: String?) -> Self { value.flatMap(Self.init(rawValue:)) ?? .source }
}

/// One view-owned entry across modes. No workspace or preferences are retained.
final class CharacterCountResultCache<Value> {
    private var source: String?
    private var revision: UInt64?
    private var mode: EditorCharacterCountMode?
    private var cached: Value?

    func value(in text: String, revision: UInt64, mode: EditorCharacterCountMode, compute: () -> Value) -> Value {
        if self.revision == revision, self.mode == mode, let source, Self.sameBytes(source, text), let cached { return cached }
        let value = compute()
        source = text; self.revision = revision; self.mode = mode; cached = value
        return value
    }
    private static func sameBytes(_ lhs: String, _ rhs: String) -> Bool {
        var left = lhs, right = rhs
        return left.withUTF8 { a in right.withUTF8 { b in
            a.count == b.count && (a.baseAddress == b.baseAddress || a.elementsEqual(b))
        } }
    }
}

/// Bounded default-catcode lexical extraction, not TeX expansion or rendered text.
/// A number is exposed only when every encountered construct is supported.
struct LatexBodyCharacterCount: Equatable {
    enum Status: String { case complete, partial, unsupported }
    enum Issue: String, Hashable {
        case unknownCommand, unknownEnvironment, dynamicTeX, unbalanced, sourceLimit, nestingLimit, commandLimit, bareSyntax
    }
    static let maximumSourceBytes = 1_048_576
    static let maximumNesting = 64
    let status: Status
    let characterCount: Int?
    let issues: [Issue]

    init(_ source: String) {
        guard source.utf8.count <= Self.maximumSourceBytes else {
            status = .unsupported; characterCount = nil; issues = [.sourceLimit]; return
        }
        var parser = BodyLexer(bytes: Array(source.utf8))
        parser.text(depth: 0, group: false)
        if parser.documentOpen { parser.mark(.unbalanced, .unsupported) }
        status = parser.status; issues = parser.issues
        characterCount = parser.status == .complete ? String(decoding: parser.output, as: UTF8.self).reduce(0) { $0 + ($1.isWhitespace ? 0 : 1) } : nil
    }
    var displayText: String {
        if let characterCount { return "Szövegtörzs: \(characterCount)" }
        return status == .partial ? "Szövegtörzs: részleges" : "Szövegtörzs: nem támogatott"
    }
    var explanation: String {
        if characterCount != nil {
            return "Lexikális szövegtörzs, whitespace nélkül. A támogatott parancsok, megjegyzések, matematikai részek és kulcsok kimaradnak. Nem TeX-expanzió vagy renderelt szövegmérés."
        }
        return "A bemenet nem fér teljesen a támogatott lexikális nyelvtanba, ezért nincs pontos szám. Ismeretlen vagy dinamikus parancs, lezáratlan szerkezet vagy méretkorlát is okozhatja."
    }
}

private struct BodyLexer {
    typealias Issue = LatexBodyCharacterCount.Issue
    typealias Status = LatexBodyCharacterCount.Status
    static let formats: Set<String> = ["textbf", "textit", "emph", "textrm", "textsf", "texttt", "textsl", "textsc", "textmd", "textup", "textnormal", "underline", "mbox", "text"]
    static let sections: Set<String> = ["part", "chapter", "section", "subsection", "subsubsection", "paragraph", "subparagraph", "caption"]
    static let keys: Set<String> = ["label", "cite", "citep", "citet", "parencite", "textcite", "autocite", "footcite", "ref", "eqref", "pageref", "autoref", "cref", "Cref", "nameref"]
    static let metadata: Set<String> = ["documentclass", "usepackage", "title", "author", "date", "bibliography", "bibliographystyle"]
    static let definitions: Set<String> = ["newcommand", "renewcommand", "providecommand", "DeclareRobustCommand"]
    static let spacing: Set<String> = ["par", "newline", "linebreak", "quad", "qquad", "enspace", "thinspace", "hfill", "vfill", "noindent"]
    static let switches: Set<String> = ["bfseries", "itshape", "normalfont", "rmfamily", "sffamily", "ttfamily"]
    static let dynamic: Set<String> = ["input", "include", "includeonly", "catcode", "csname", "endcsname", "expandafter", "edef", "xdef", "gdef", "def", "let", "futurelet", "scantokens", "write", "read", "if", "ifnum", "ifx", "ifdefined", "else", "fi", "verb", "verbatim", "makeatletter", "makeatother"]
    static let mathEnvironments: Set<String> = ["math", "displaymath", "equation", "equation*", "align", "align*", "alignat", "alignat*", "gather", "gather*", "multline", "multline*", "eqnarray", "eqnarray*", "aligned", "split"]
    static let mathWords: Set<String> = ["frac", "sqrt", "alpha", "beta", "gamma", "delta", "epsilon", "theta", "lambda", "mu", "pi", "sigma", "phi", "omega", "sin", "cos", "tan", "log", "ln", "exp", "left", "right", "big", "Big", "bigg", "Bigg", "sum", "prod", "int", "lim", "cdot", "times", "leq", "geq", "neq", "infty", "overline", "underbrace", "vec", "hat", "bar", "mathrm", "mathbf", "mathit", "mathsf", "mathtt", "operatorname"]
    let bytes: [UInt8]
    var position = 0
    var output: [UInt8] = []
    var status: Status = .complete
    var issues: [Issue] = []
    var documentOpen = false
    var documentSeen = false
    var current: UInt8? { position < bytes.count ? bytes[position] : nil }

    mutating func mark(_ issue: Issue, _ severity: Status) {
        if !issues.contains(issue), issues.count < 16 { issues.append(issue) }
        if severity == .unsupported || status == .complete { status = severity }
    }
    mutating func limit() { mark(.nestingLimit, .unsupported); position = bytes.count }
    static func letter(_ byte: UInt8) -> Bool { (65...90).contains(byte) || (97...122).contains(byte) }
    static func space(_ byte: UInt8) -> Bool { [9, 10, 11, 12, 13, 32].contains(byte) }
    mutating func comment() { while let byte = current, byte != 10 && byte != 13 { position += 1 } }
    mutating func trivia() {
        while let byte = current {
            if Self.space(byte) { position += 1 }
            else if byte == 37 { comment() }
            else { return }
        }
    }
    mutating func scalar() {
        guard let byte = current else { return }
        let length = byte < 128 ? 1 : (byte < 224 ? 2 : (byte < 240 ? 3 : 4))
        position = min(bytes.count, position + length)
    }
    mutating func word() -> String {
        let start = position
        while let byte = current, Self.letter(byte) {
            position += 1
            if position - start > 128 { mark(.commandLimit, .unsupported); position = bytes.count; return "" }
        }
        return String(decoding: bytes[start..<position], as: UTF8.self)
    }
    /// Balanced opaque arguments; comments and escaped delimiters cannot close them.
    mutating func opaque(_ open: UInt8 = 123, _ close: UInt8 = 125, depth parentDepth: Int) -> [UInt8]? {
        trivia()
        guard current == open else { mark(.unbalanced, .unsupported); return nil }
        position += 1; let start = position
        var depth = 1, braces = 0
        guard parentDepth + depth <= LatexBodyCharacterCount.maximumNesting else { limit(); return nil }
        while let byte = current {
            if byte == 92 { position += 1; scalar(); continue }
            if byte == 37 { comment(); continue }
            if open == 91 {
                if byte == 123 { braces += 1 }
                else if byte == 125 { braces -= 1; if braces < 0 { mark(.unbalanced, .unsupported); return nil } }
                if braces == 0 {
                    if byte == open { depth += 1 }
                    else if byte == close { depth -= 1 }
                }
            } else {
                if byte == open { depth += 1 }
                else if byte == close { depth -= 1 }
            }
            if parentDepth + depth + braces > LatexBodyCharacterCount.maximumNesting { limit(); return nil }
            if depth == 0 { let end = position; position += 1; return Array(bytes[start..<end]) }
            position += 1
        }
        mark(.unbalanced, .unsupported); return nil
    }
    mutating func optionalArguments(depth: Int) {
        trivia(); var count = 0
        while current == 91 {
            _ = opaque(91, 93, depth: depth); count += 1
            if count > 2 { mark(.bareSyntax, .unsupported); return }
            trivia()
        }
    }
    mutating func argument(depth: Int) {
        trivia()
        guard current == 123 else { mark(.unbalanced, .unsupported); return }
        position += 1; text(depth: depth + 1, group: true)
    }
    mutating func text(depth: Int, group: Bool) {
        guard depth <= LatexBodyCharacterCount.maximumNesting else { limit(); return }
        while let byte = current {
            switch byte {
            case 37: comment()
            case 92: position += 1; command(depth: depth)
            case 123: position += 1; text(depth: depth + 1, group: true)
            case 125:
                position += 1
                if group { return }
                mark(.unbalanced, .unsupported)
            case 36:
                position += 1
                let display = current == 36
                if display { position += 1 }
                math(dollarWidth: display ? 2 : 1, closingSymbol: nil, depth: depth)
            case 126: position += 1 // TeX nonbreaking space.
            case 35, 38, 94, 95: mark(.bareSyntax, .unsupported); position += 1
            case 0...8, 14...31: mark(.bareSyntax, .unsupported); position += 1
            default: output.append(byte); position += 1
            }
        }
        if group { mark(.unbalanced, .unsupported) }
    }
    mutating func command(depth: Int) {
        guard let byte = current else { mark(.unbalanced, .unsupported); return }
        if !Self.letter(byte) {
            position += 1
            switch byte {
            case 37, 38, 35, 36, 95, 123, 125: output.append(byte)
            case 92: // Explicit line break, not a literal backslash.
                trivia()
                if current == 42 { position += 1 }
                trivia()
                if current == 91 { _ = opaque(91, 93, depth: depth) }
            case 40: math(dollarWidth: 0, closingSymbol: 41, depth: depth)
            case 91: math(dollarWidth: 0, closingSymbol: 93, depth: depth)
            case 41, 93: mark(.unbalanced, .unsupported)
            case 32, 9, 10, 13, 44, 59, 58, 33: break
            default:
                if byte >= 128 { position -= 1; scalar() }
                mark(.unknownCommand, .partial)
            }
            return
        }
        let name = word()
        if Self.dynamic.contains(name) { mark(.dynamicTeX, .unsupported); skipArguments(depth: depth); return }
        if Self.formats.contains(name) { argument(depth: depth); return }
        if Self.sections.contains(name) {
            if current == 42 { position += 1 }
            optionalArguments(depth: depth); argument(depth: depth); return
        }
        if Self.keys.contains(name) {
            if current == 42 { position += 1 }
            optionalArguments(depth: depth); _ = opaque(depth: depth); return
        }
        if Self.metadata.contains(name) { optionalArguments(depth: depth); _ = opaque(depth: depth); return }
        if Self.definitions.contains(name) {
            if current == 42 { position += 1 }
            _ = opaque(depth: depth); optionalArguments(depth: depth); _ = opaque(depth: depth); return
        }
        if ["setlength", "addtolength", "setcounter", "addtocounter"].contains(name) { _ = opaque(depth: depth); _ = opaque(depth: depth); return }
        if name == "newlength" { _ = opaque(depth: depth); return }
        if name == "linebreak" {
            trivia()
            if current == 91 {
                guard let strength = opaque(91, 93, depth: depth) else { return }
                if strength.count != 1 || !(48...52).contains(strength[0]) { mark(.bareSyntax, .unsupported) }
            }
            return
        }
        if Self.spacing.contains(name) || Self.switches.contains(name) { return }
        if let symbol = ["textbackslash": UInt8(92), "textasciitilde": 126, "textasciicircum": 94][name] { output.append(symbol); return }
        if name == "begin" || name == "end" {
            guard let raw = opaque(depth: depth) else { return }
            let environment = String(decoding: raw, as: UTF8.self)
            if environment == "document" {
                if name == "begin", !documentSeen { documentSeen = true; documentOpen = true }
                else if name == "end", documentOpen, depth == 0 { documentOpen = false; position = bytes.count }
                else { mark(.unbalanced, .unsupported) }
            } else if name == "begin" {
                let math = Self.mathEnvironments.contains(environment)
                if !math { mark(.unknownEnvironment, .partial) }
                environmentBody(environment, math: math, depth: depth)
            } else { mark(.unbalanced, .unsupported) }
            return
        }
        mark(.unknownCommand, .partial); skipArguments(depth: depth)
    }
    mutating func skipArguments(depth: Int) {
        optionalArguments(depth: depth); trivia()
        while current == 123 { _ = opaque(depth: depth); trivia() }
    }
    mutating func mathWord(_ name: String) {
        if Self.dynamic.contains(name) || Self.definitions.contains(name) { mark(.dynamicTeX, .unsupported) }
        else if !Self.mathWords.contains(name) && !Self.formats.contains(name) && !Self.keys.contains(name) && !Self.spacing.contains(name) {
            mark(.unknownCommand, .partial)
        }
    }
    mutating func math(dollarWidth: Int, closingSymbol: UInt8?, depth: Int) {
        var braces = 0
        while let byte = current {
            if byte == 37 { comment(); continue }
            if dollarWidth > 0, byte == 36 {
                if dollarWidth == 1 || position + 1 < bytes.count && bytes[position + 1] == 36 {
                    position += dollarWidth
                    if braces != 0 { mark(.unbalanced, .unsupported) }
                    return
                }
                mark(.unbalanced, .unsupported)
            }
            if byte == 92 {
                position += 1
                if let closingSymbol, current == closingSymbol {
                    position += 1; if braces != 0 { mark(.unbalanced, .unsupported) }; return
                }
                if let next = current, Self.letter(next) {
                    let name = word()
                    if name == "begin" || name == "end" { mark(.bareSyntax, .unsupported) }
                    else { mathWord(name) }
                } else {
                    if let symbol = current, ![9, 10, 13, 32, 33, 35, 36, 37, 38, 44, 47, 58, 59, 92, 95, 123, 124, 125].contains(symbol) { mark(.unknownCommand, .partial) }
                    scalar()
                }
                continue
            }
            if byte == 123 { braces += 1 }
            if byte == 125 { braces -= 1; if braces < 0 { mark(.unbalanced, .unsupported) } }
            if depth + braces > LatexBodyCharacterCount.maximumNesting { limit(); return }
            position += 1
        }
        mark(.unbalanced, .unsupported)
    }
    mutating func environmentBody(_ name: String, math: Bool, depth: Int) {
        var stack = [name], braces = 0
        guard depth + stack.count <= LatexBodyCharacterCount.maximumNesting else { limit(); return }
        while let byte = current {
            if byte == 37 { comment(); continue }
            if byte == 92 {
                position += 1
                guard let next = current else { break }
                if Self.letter(next) {
                    let command = word()
                    if command == "begin" || command == "end" {
                        // An end command identifies the level being closed.
                        let contextDepth = depth + braces + stack.count - (command == "end" ? 1 : 0)
                        guard let raw = opaque(depth: contextDepth) else { return }
                        let environment = String(decoding: raw, as: UTF8.self)
                        if command == "begin" {
                            stack.append(environment)
                            if math && !Self.mathEnvironments.contains(environment) { mark(.unknownEnvironment, .partial) }
                        } else {
                            guard stack.last == environment else { mark(.unbalanced, .unsupported); return }
                            stack.removeLast()
                            if stack.isEmpty { if braces != 0 { mark(.unbalanced, .unsupported) }; return }
                        }
                        if depth + stack.count + braces > LatexBodyCharacterCount.maximumNesting { limit(); return }
                    } else if math { mathWord(command) }
                } else {
                    if math, let symbol = current, ![9, 10, 13, 32, 33, 35, 36, 37, 38, 44, 47, 58, 59, 92, 95, 123, 124, 125].contains(symbol) { mark(.unknownCommand, .partial) }
                    scalar()
                }
                continue
            }
            if byte == 123 { braces += 1 }
            if byte == 125 { braces -= 1; if braces < 0 { mark(.unbalanced, .unsupported) } }
            if depth + stack.count + braces > LatexBodyCharacterCount.maximumNesting { limit(); return }
            position += 1
        }
        mark(.unbalanced, .unsupported)
    }
}
