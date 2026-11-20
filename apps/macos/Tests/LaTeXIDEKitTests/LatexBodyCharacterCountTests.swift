import Foundation
#if !LATEX_BODY_CLI
import XCTest
@testable import LaTeXIDEKit
#endif

enum LatexBodyCountControls {
    struct Failure: Error, CustomStringConvertible { let description: String }
    static func expect(_ value: @autoclosure () -> Bool, _ message: String) throws {
        guard value() else { throw Failure(description: message) }
    }
    @discardableResult static func count(_ source: String, _ expected: Int?, _ status: LatexBodyCharacterCount.Status) throws -> LatexBodyCharacterCount {
        let actual = LatexBodyCharacterCount(source)
        let row: [String: Any] = ["source_UTF8_bytes": source.utf8.count, "status": actual.status.rawValue,
                                  "count": actual.characterCount as Any? ?? NSNull(), "issues": actual.issues.map(\.rawValue)]
        let data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
        print("UI69_OBSERVATION " + String(decoding: data, as: UTF8.self))
        try expect(actual.status == status, "Expected status \(status), got \(actual.status)")
        try expect(actual.characterCount == expected, "Expected count \(String(describing: expected)), got \(actual)")
        return actual
    }
    static func testPlainAndCacheKey() throws {
        try count("alma fa", 6, .complete); try count("", 0, .complete)
        try expect(EditorCharacterCountMode.restored(nil) == .source, "Raw source default changed")
        try expect(EditorCharacterCountMode.restored("invalid") == .source, "Invalid stored mode fallback changed")
        try expect(EditorCharacterCountMode.restored("latexBody") == .latexBody, "Body mode failed to restore")
        let cache = CharacterCountResultCache<Int>(); var calls = 0
        func get(_ source: String, _ revision: UInt64, _ mode: EditorCharacterCountMode) -> Int {
            cache.value(in: source, revision: revision, mode: mode) { calls += 1; return calls }
        }
        try expect(get("a", 1, .source) == 1 && get("a", 1, .source) == 1, "Caret-only cache miss")
        try expect(get("a", 2, .source) == 2, "Revision key ignored")
        try expect(get("a", 2, .latexBody) == 3, "Mode key ignored")
        try expect(get("b", 2, .latexBody) == 4, "Text key ignored")
        try expect(get("a", 1, .source) == 5 && calls == 5, "Cache retained multiple entries")
    }
    static func testUnicodeGraphemes() throws {
        try count("e\u{301} 👩‍💻 🇭🇺", 3, .complete)
        try count("é", 1, .complete); try count("e\u{301}", 1, .complete)
        let cache = CharacterCountResultCache<Int>(); var calls = 0
        _ = cache.value(in: "é", revision: 1, mode: .latexBody) { calls += 1; return calls }
        _ = cache.value(in: "e\u{301}", revision: 1, mode: .latexBody) { calls += 1; return calls }
        try expect(calls == 2, "Literal UTF8 text key collapsed NFC/NFD")
    }
    static func testAllWhitespace() throws {
        try count("a\tb\r\nc\u{a0}d", 4, .complete)
        try count(" \t\n\r\n\u{a0}\u{2003}", 0, .complete)
    }
    static func testNestedTextCommands() throws {
        try count(#"\textbf{alma \emph{fa}}"#, 6, .complete)
        try count("e\\textbf{\u{301}}", 1, .complete)
    }
    static func testSectionArguments() throws { try count(#"\section[TOC]{Első cím} \subsection*{Terv}"#, 11, .complete) }
    static func testCommentsAndEscapedPercent() throws {
        try count("A% hidden\nB \\% C", 4, .complete)
        try count("\\textbf{a% }\nb}", 2, .complete)
    }
    static func testOpaqueMath() throws {
        try count(#"A $x+y$ B $$z$$ C \(q\) D \[w\] E \begin{equation}f=0\end{equation}"#, 5, .complete)
        try count(#"$\frac{x}{2}+\alpha$"#, 0, .complete)
    }
    static func testCiteLabelRefKeys() throws {
        try count(#"Szó\label{sec:a}\cite[p.2]{k1,k2}\ref{sec:a}\eqref{eq:x}\pageref{p} vége"#, 7, .complete)
    }
    static func testDeclarationsAndDocument() throws {
        try count(#"\documentclass[11pt]{article}\usepackage{x}\newcommand{\foo}[1]{hidden #1}\begin{document}Alma\end{document}"#, 4, .complete)
    }
    static func testLiteralEscapes() throws {
        try count(##"\% \& \# \$ \_ \{ \} \textbackslash{} ~ \\"##, 8, .complete)
    }
    static func testUnknownAndDynamicFailClosed() throws {
        let partial = try count(#"a \unknown{hidden} b"#, nil, .partial)
        try expect(partial.displayText == "Szövegtörzs: részleges", "Unknown input exposed an exact-looking number")
        try count(#"$\mystery{x}$"#, nil, .partial)
        try count(#"\(x\?\)"#, nil, .partial)
        try count(#"\input{other}"#, nil, .unsupported)
        try count(#"\catcode`\%=12"#, nil, .unsupported)
        try count(#"\begin{mystery}text\end{mystery}"#, nil, .partial)
        try count(#"\begin{equation}\input{other}\end{equation}"#, nil, .unsupported)
    }
    static func testUnbalancedAndBounds() throws {
        for source in [#"\textbf{open"#, "$open", #"\[open"#, "text}", #"\begin{document}open"#,
                       #"\begin{equation}x\end{align}"#, #"\cite[open"#] { try count(source, nil, .unsupported) }
        try count(String(repeating: "{", count: 64) + "x" + String(repeating: "}", count: 64), 1, .complete)
        try count(String(repeating: "{", count: 65) + "x" + String(repeating: "}", count: 65), nil, .unsupported)
        try count(String(repeating: "x", count: 1_048_577), nil, .unsupported)
    }
    static func testOpaqueArgumentsRespectCombinedNestingLimit() throws {
        func grouped(_ levels: Int, _ body: String) -> String {
            String(repeating: "{", count: levels) + body + String(repeating: "}", count: levels)
        }
        try count(grouped(63, #"\cite{k}"#), 0, .complete)
        try count(grouped(64, #"\cite{k}"#), nil, .unsupported)
        try count(grouped(62, #"\cite{{k}}"#), 0, .complete)
        try count(grouped(63, #"\cite{{k}}"#), nil, .unsupported)
        try count(grouped(62, #"\cite[{note}]{k}"#), 0, .complete)
        try count(grouped(63, #"\cite[{note}]{k}"#), nil, .unsupported)
    }
    static func testMathAndEnvironmentsShareOuterNestingBudget() throws {
        func grouped(_ levels: Int, _ body: String) -> String {
            String(repeating: "{", count: levels) + body + String(repeating: "}", count: levels)
        }
        try count(grouped(63, "$ {x} $"), 0, .complete)
        try count(grouped(64, "$ {x} $"), nil, .unsupported)
        try count(grouped(62, #"\begin{equation}{x}\end{equation}"#), 0, .complete)
        try count(grouped(63, #"\begin{equation}{x}\end{equation}"#), nil, .unsupported)
        try count(grouped(63, #"\begin{equation}x\end{equation}"#), 0, .complete)
        let open = #"\begin{equation}"#, close = #"\end{equation}"#
        try count(String(repeating: open, count: 64) + "x" + String(repeating: close, count: 64), 0, .complete)
        try count(String(repeating: open, count: 65) + "x" + String(repeating: close, count: 65), nil, .unsupported)
    }
    static func testStarredAndLengthLineBreaksDoNotCountSyntax() throws {
        for source in [#"a\\b"#, #"a\\*b"#, #"a\\[2pt]b"#, #"a\\*[2pt]b"#, #"a\\ * [2pt]b"#] {
            try count(source, 2, .complete)
        }
        try count(#"a\\[2pt][x]b"#, 5, .complete)
        try count(#"a\\*[2pt"#, nil, .unsupported)
    }
    static func testLinebreakStrengthArgumentsDoNotCountSyntax() throws {
        try count(#"a\linebreak b"#, 2, .complete)
        for strength in 0...4 { try count("a\\linebreak[\(strength)]b", 2, .complete) }
        for source in [#"a\linebreak[5]b"#, #"a\linebreak[-1]b"#, #"a\linebreak[x]b"#, #"a\linebreak[44]b"#, #"a\linebreak[4"#] {
            try count(source, nil, .unsupported)
        }
    }
    static func testLineBreakLengthExpressionsFailClosed() throws {
        for control in [#"\\"#, #"\\*"#] {
            try count("a" + control + #"[\input{other}]b"#, nil, .unsupported)
            try count("a" + control + #"[\mystery]b"#, nil, .partial)
            try count("a" + control + #"[2pt+\input{other}]b"#, nil, .unsupported)
            try count("a" + control + #"[2pt]b"#, 2, .complete)
        }
        try count(#"a\\*[2pt% \input{ignored}"# + "\n]b", 2, .complete)
    }
    static func testLineBreakLiteralLengthSubset() throws {
        for length in ["-0.5em", "+2pt", ".5ex", "2 pt", "2,5cm", " 2pt "] {
            try count(#"a\\*["# + length + "]b", 2, .complete)
        }
        for length in ["", "nonsense", "2p", "2pt plus1pt", "2pt junk"] {
            try count(#"a\\*["# + length + "]b", nil, .unsupported)
        }
    }
    static let methods: [(String, () throws -> Void)] = [
        ("testPlainAndCacheKey", testPlainAndCacheKey), ("testUnicodeGraphemes", testUnicodeGraphemes),
        ("testAllWhitespace", testAllWhitespace), ("testNestedTextCommands", testNestedTextCommands),
        ("testSectionArguments", testSectionArguments), ("testCommentsAndEscapedPercent", testCommentsAndEscapedPercent),
        ("testOpaqueMath", testOpaqueMath), ("testCiteLabelRefKeys", testCiteLabelRefKeys),
        ("testDeclarationsAndDocument", testDeclarationsAndDocument), ("testLiteralEscapes", testLiteralEscapes),
        ("testUnknownAndDynamicFailClosed", testUnknownAndDynamicFailClosed), ("testUnbalancedAndBounds", testUnbalancedAndBounds),
        ("testOpaqueArgumentsRespectCombinedNestingLimit", testOpaqueArgumentsRespectCombinedNestingLimit),
        ("testMathAndEnvironmentsShareOuterNestingBudget", testMathAndEnvironmentsShareOuterNestingBudget),
        ("testStarredAndLengthLineBreaksDoNotCountSyntax", testStarredAndLengthLineBreaksDoNotCountSyntax),
        ("testLinebreakStrengthArgumentsDoNotCountSyntax", testLinebreakStrengthArgumentsDoNotCountSyntax),
        ("testLineBreakLengthExpressionsFailClosed", testLineBreakLengthExpressionsFailClosed),
        ("testLineBreakLiteralLengthSubset", testLineBreakLiteralLengthSubset)
    ]
}

#if !LATEX_BODY_CLI
final class LatexBodyCharacterCountTests: XCTestCase {
    func testPlainAndCacheKey() throws { try LatexBodyCountControls.testPlainAndCacheKey() }
    func testUnicodeGraphemes() throws { try LatexBodyCountControls.testUnicodeGraphemes() }
    func testAllWhitespace() throws { try LatexBodyCountControls.testAllWhitespace() }
    func testNestedTextCommands() throws { try LatexBodyCountControls.testNestedTextCommands() }
    func testSectionArguments() throws { try LatexBodyCountControls.testSectionArguments() }
    func testCommentsAndEscapedPercent() throws { try LatexBodyCountControls.testCommentsAndEscapedPercent() }
    func testOpaqueMath() throws { try LatexBodyCountControls.testOpaqueMath() }
    func testCiteLabelRefKeys() throws { try LatexBodyCountControls.testCiteLabelRefKeys() }
    func testDeclarationsAndDocument() throws { try LatexBodyCountControls.testDeclarationsAndDocument() }
    func testLiteralEscapes() throws { try LatexBodyCountControls.testLiteralEscapes() }
    func testUnknownAndDynamicFailClosed() throws { try LatexBodyCountControls.testUnknownAndDynamicFailClosed() }
    func testUnbalancedAndBounds() throws { try LatexBodyCountControls.testUnbalancedAndBounds() }
    func testOpaqueArgumentsRespectCombinedNestingLimit() throws { try LatexBodyCountControls.testOpaqueArgumentsRespectCombinedNestingLimit() }
    func testMathAndEnvironmentsShareOuterNestingBudget() throws { try LatexBodyCountControls.testMathAndEnvironmentsShareOuterNestingBudget() }
    func testStarredAndLengthLineBreaksDoNotCountSyntax() throws { try LatexBodyCountControls.testStarredAndLengthLineBreaksDoNotCountSyntax() }
    func testLinebreakStrengthArgumentsDoNotCountSyntax() throws { try LatexBodyCountControls.testLinebreakStrengthArgumentsDoNotCountSyntax() }
    func testLineBreakLengthExpressionsFailClosed() throws { try LatexBodyCountControls.testLineBreakLengthExpressionsFailClosed() }
    func testLineBreakLiteralLengthSubset() throws { try LatexBodyCountControls.testLineBreakLiteralLengthSubset() }
}
#endif
