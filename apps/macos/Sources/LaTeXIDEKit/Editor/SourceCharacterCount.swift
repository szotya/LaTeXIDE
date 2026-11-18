/// Counts the complete editor source, including LaTeX commands and comments.
/// Each Swift Character is one extended grapheme cluster, including CRLF.
struct SourceCharacterCount: Equatable {
    let includingWhitespace: Int
    let excludingWhitespace: Int

    init(_ source: String) {
        var total = 0
        var withoutWhitespace = 0
        for character in source {
            total += 1
            if !character.isWhitespace { withoutWhitespace += 1 }
        }
        includingWhitespace = total
        excludingWhitespace = withoutWhitespace
    }
}

/// A headless projection that shares the workspace's live preference object.
@MainActor struct SourceCharacterCountPresentation {
    let preferences: EditorPreferences

    func count(in source: String) -> SourceCharacterCount? {
        guard preferences.showSourceCharacterCount else { return nil }
        return SourceCharacterCount(source)
    }
}
