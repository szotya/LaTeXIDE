import Foundation
import XCTest
@testable import LaTeXIDEKit

private final class MemoryPreferenceStorage: EditorPreferenceStorage {
    var values: [String: Any]
    var writtenKeys: [String] = []
    init(_ values: [String: Any] = [:]) { self.values = values }
    func object(forKey key: String) -> Any? { values[key] }
    func string(forKey key: String) -> String? { values[key] as? String }
    func double(forKey key: String) -> Double { (values[key] as? NSNumber)?.doubleValue ?? 0 }
    func bool(forKey key: String) -> Bool { (values[key] as? NSNumber)?.boolValue ?? false }
    func set(_ value: Any?, forKey key: String) {
        values[key] = value
        writtenKeys.append(key)
    }
}

final class SourceCharacterCountTests: XCTestCase {
    private func assertCount(_ count: SourceCharacterCount?, _ total: Int, _ nonWhitespace: Int,
                             file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(count?.includingWhitespace, total, file: file, line: line)
        XCTAssertEqual(count?.excludingWhitespace, nonWhitespace, file: file, line: line)
    }

    func testEmptySource() { assertCount(SourceCharacterCount(""), 0, 0) }
    func testAlmaFa() { assertCount(SourceCharacterCount("alma fa"), 7, 6) }
    func testAppendedExclamation() { assertCount(SourceCharacterCount("alma fa!"), 8, 7) }
    func testWhitespaceOnlyIncludingCRLF() { assertCount(SourceCharacterCount(" \t\n\r\n"), 4, 0) }
    func testTabAndNewline() { assertCount(SourceCharacterCount("a\tb\nc"), 5, 3) }
    func testCombiningAccentGrapheme() { assertCount(SourceCharacterCount("e\u{301}"), 1, 1) }
    func testPrecomposedAccentGrapheme() { assertCount(SourceCharacterCount("\u{e9}"), 1, 1) }
    func testZWJEmojiGrapheme() { assertCount(SourceCharacterCount("👩‍💻"), 1, 1) }
    func testRegionalIndicatorFlagGrapheme() { assertCount(SourceCharacterCount("🇭🇺"), 1, 1) }
    func testUnicodeSpaces() { assertCount(SourceCharacterCount("a\u{a0}b\u{2003}c"), 5, 3) }
    func testRawLatexIncludesCommandAndComment() { assertCount(SourceCharacterCount("\\textbf{alma} % x\n"), 18, 15) }
    func testLargeLocalGraphemeString() {
        let source = String(repeating: "a e\u{301} 👩‍💻\t\n", count: 65_536)
        XCTAssertEqual(source.utf8.count, 1_245_184)
        assertCount(SourceCharacterCount(source), 458_752, 196_608)
    }

    @MainActor func testFreshUnsavedStoreInput() {
        // Seed the real presentation store without opening a Runtime, project or UI.
        let store = WorkspaceStore()
        let presentation = SourceCharacterCountPresentation(preferences: EditorPreferences(storage: MemoryPreferenceStorage()))
        store.selection = "main.tex"; store.text = "alma fa"
        assertCount(presentation.count(in: store.text), 7, 6)
        store.text += "!"; store.dirty = true; store.revision = 2
        assertCount(presentation.count(in: store.text), 8, 7)
        XCTAssertTrue(store.dirty); XCTAssertEqual(store.revision, 2)
        XCTAssertEqual(store.text, "alma fa!")
    }

    @MainActor func testFileSwitchUsesCurrentStoreText() {
        let store = WorkspaceStore()
        let presentation = SourceCharacterCountPresentation(preferences: EditorPreferences(storage: MemoryPreferenceStorage()))
        store.selection = "first.tex"; store.text = "alma fa!"
        assertCount(presentation.count(in: store.text), 8, 7)
        store.selection = "second.tex"; store.text = "e\u{301} 🇭🇺"
        assertCount(presentation.count(in: store.text), 3, 2)
        store.selection = "empty.tex"; store.text = ""
        assertCount(presentation.count(in: store.text), 0, 0)
        XCTAssertEqual(store.selection, "empty.tex")
    }

    @MainActor func testMemoryPreferencesKeepExistingDefaults() {
        let memory = MemoryPreferenceStorage()
        let preferences = EditorPreferences(storage: memory)
        XCTAssertTrue(preferences.showSourceCharacterCount)
        XCTAssertEqual(preferences.appearance, .system); XCTAssertEqual(preferences.paletteID, "graphite")
        XCTAssertEqual(preferences.fontSize, 14); XCTAssertTrue(preferences.completionEnabled)
        XCTAssertEqual(preferences.sidebarWidth, 230); XCTAssertEqual(preferences.outputHeight, 160)
        XCTAssertEqual(preferences.previewFraction, 0.42); XCTAssertEqual(preferences.inspectorTab, .pdf)
        XCTAssertTrue(memory.writtenKeys.isEmpty); XCTAssertTrue(memory.values.isEmpty)
    }

    @MainActor func testStoredCounterChoicesAndMemoryRecreation() {
        for stored in [false, true] {
            let memory = MemoryPreferenceStorage(["ide.showSourceCharacterCount": stored])
            let preferences = EditorPreferences(storage: memory)
            XCTAssertEqual(preferences.showSourceCharacterCount, stored)
            XCTAssertTrue(memory.writtenKeys.isEmpty)
            preferences.showSourceCharacterCount = !stored
            let restored = EditorPreferences(storage: memory)
            XCTAssertEqual(restored.showSourceCharacterCount, !stored)
            XCTAssertEqual(memory.writtenKeys, ["ide.showSourceCharacterCount"])
            XCTAssertEqual(Set(memory.values.keys), ["ide.showSourceCharacterCount"])
        }
    }

    @MainActor func testExistingPreferenceWritesKeepKeysAndValues() {
        let memory = MemoryPreferenceStorage()
        let preferences = EditorPreferences(storage: memory)
        preferences.appearance = .dark; preferences.paletteID = "dune"
        preferences.fontSize = 19; preferences.completionEnabled = false
        preferences.sidebarWidth = 310; preferences.outputHeight = 240
        preferences.previewFraction = 0.48; preferences.inspectorTab = .assistant
        preferences.showSourceCharacterCount = false
        XCTAssertEqual(memory.writtenKeys, ["ide.appearance", "ide.palette", "ide.fontSize", "ide.completion",
            "ide.sidebarWidth", "ide.outputHeight", "ide.previewFraction", "ide.inspectorTab", "ide.showSourceCharacterCount"])
        XCTAssertEqual(memory.values["ide.appearance"] as? String, "dark")
        XCTAssertEqual(memory.values["ide.palette"] as? String, "dune")
        XCTAssertEqual(memory.values["ide.inspectorTab"] as? String, "assistant")
        let restored = EditorPreferences(storage: memory)
        XCTAssertEqual(restored.appearance, .dark); XCTAssertEqual(restored.paletteID, "dune")
        XCTAssertEqual(restored.fontSize, 19); XCTAssertFalse(restored.completionEnabled)
        XCTAssertEqual(restored.sidebarWidth, 310); XCTAssertEqual(restored.outputHeight, 240)
        XCTAssertEqual(restored.previewFraction, 0.48); XCTAssertEqual(restored.inspectorTab, .assistant)
        XCTAssertFalse(restored.showSourceCharacterCount); XCTAssertEqual(memory.writtenKeys.count, 9)
    }

    @MainActor func testPresentationRetainsSamePreferencesInstance() {
        let preferences = EditorPreferences(storage: MemoryPreferenceStorage())
        let otherPreferences = EditorPreferences(storage: MemoryPreferenceStorage())
        let presentation = SourceCharacterCountPresentation(preferences: preferences)
        XCTAssertTrue(presentation.preferences === preferences)
        XCTAssertFalse(presentation.preferences === otherPreferences)
        otherPreferences.showSourceCharacterCount = false
        assertCount(presentation.count(in: "alma fa"), 7, 6)
        preferences.showSourceCharacterCount = false
        XCTAssertNil(presentation.count(in: "alma fa"))
        preferences.showSourceCharacterCount = true
        assertCount(presentation.count(in: "alma fa!"), 8, 7)
    }

    @MainActor func testTogglePreservesStoreAndSelection() {
        let store = WorkspaceStore()
        store.text = "e\u{301} 👩‍💻\n"; store.selection = "dirty.tex"
        store.dirty = true; store.revision = 42; store.canUndo = true; store.canRedo = false
        store.editorSelection = NSRange(location: 2, length: 1)
        store.caretLine = 2; store.caretColumn = 3; store.editorFocusVersion = 7
        let sourceBytes = Array(store.text.utf8)
        let memory = MemoryPreferenceStorage()
        let preferences = EditorPreferences(storage: memory)
        let presentation = SourceCharacterCountPresentation(preferences: preferences)
        preferences.showSourceCharacterCount = false
        XCTAssertNil(presentation.count(in: store.text))
        preferences.showSourceCharacterCount = true
        assertCount(presentation.count(in: store.text), 4, 2)
        XCTAssertEqual(Array(store.text.utf8), sourceBytes)
        XCTAssertEqual(store.selection, "dirty.tex"); XCTAssertTrue(store.dirty)
        XCTAssertEqual(store.revision, 42); XCTAssertTrue(store.canUndo); XCTAssertFalse(store.canRedo)
        XCTAssertEqual(store.editorSelection, NSRange(location: 2, length: 1))
        XCTAssertEqual(store.caretLine, 2); XCTAssertEqual(store.caretColumn, 3)
        XCTAssertEqual(store.editorFocusVersion, 7)
        XCTAssertEqual(memory.writtenKeys, ["ide.showSourceCharacterCount", "ide.showSourceCharacterCount"])
        // These are seeded model invariants; actual Runtime history/live undo is separate acceptance.
    }

    @MainActor func testExistingPreferenceFallbacksAndClamps() {
        let memory = MemoryPreferenceStorage(["ide.appearance": "invalid", "ide.palette": "custom",
            "ide.fontSize": 3.0, "ide.completion": false, "ide.sidebarWidth": 900.0,
            "ide.outputHeight": -8.0, "ide.previewFraction": Double.infinity, "ide.inspectorTab": "invalid"])
        let preferences = EditorPreferences(storage: memory)
        XCTAssertEqual(preferences.appearance, .system); XCTAssertEqual(preferences.paletteID, "custom")
        XCTAssertEqual(preferences.fontSize, 11); XCTAssertFalse(preferences.completionEnabled)
        XCTAssertEqual(preferences.sidebarWidth, 420); XCTAssertEqual(preferences.outputHeight, 95)
        XCTAssertEqual(preferences.previewFraction, 0.42); XCTAssertEqual(preferences.inspectorTab, .pdf)
        XCTAssertTrue(preferences.showSourceCharacterCount); XCTAssertTrue(memory.writtenKeys.isEmpty)
        let invalid = MemoryPreferenceStorage(["ide.fontSize": 100.0, "ide.sidebarWidth": Double.nan,
            "ide.outputHeight": Double.infinity, "ide.previewFraction": -5.0])
        let fallback = EditorPreferences(storage: invalid)
        XCTAssertEqual(fallback.fontSize, 22); XCTAssertEqual(fallback.sidebarWidth, 230)
        XCTAssertEqual(fallback.outputHeight, 160); XCTAssertEqual(fallback.previewFraction, 0.25)
        XCTAssertTrue(fallback.completionEnabled); XCTAssertTrue(invalid.writtenKeys.isEmpty)
    }
}
