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

/// One view-owned entry; caret-only updates reuse the unchanged source count.
@MainActor final class SourceCharacterCountCache {
    private let compute: (String) -> SourceCharacterCount
    private var source: String?
    private var value: SourceCharacterCount?

    init(compute: @escaping (String) -> SourceCharacterCount = SourceCharacterCount.init) {
        self.compute = compute
    }

    func count(in source: String) -> SourceCharacterCount {
        if self.source == source, let value { return value }
        let value = compute(source)
        self.source = source
        self.value = value
        return value
    }
}

/// A headless projection that shares the workspace's live preference object.
@MainActor struct SourceCharacterCountPresentation {
    let preferences: EditorPreferences
    private let cache: SourceCharacterCountCache

    init(preferences: EditorPreferences, cache: SourceCharacterCountCache? = nil) {
        self.preferences = preferences
        self.cache = cache ?? SourceCharacterCountCache()
    }

    func count(in source: String) -> SourceCharacterCount? {
        guard preferences.showSourceCharacterCount else { return nil }
        return cache.count(in: source)
    }
}
