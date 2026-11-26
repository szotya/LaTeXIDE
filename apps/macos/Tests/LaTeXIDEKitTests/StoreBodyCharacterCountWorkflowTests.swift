import Foundation
import XCTest
@testable import LaTeXIDEKit

// Six synchronous Store-owned Runtime workflows; no app, window or RunLoop pumping.
final class StoreBodyCharacterCountWorkflowTests: XCTestCase {
    @MainActor @objc func testDirtyUnicodeEditUsesActualStore() {
        workflow { store, root, trace in
            let counts = StoreWorkflowCounts()
            try append("e\u{301}👩‍💻", to: store)
            try storeWorkflowRequire(Data(store.text.utf8) == Data("alma fae\u{301}👩‍💻".utf8), "Unicode bytes changed")
            try storeWorkflowRequire(store.revision == 2 && store.dirty && store.canUndo, "Actual dirty/revision/history mismatch")
            try counts.assert(store, rawTotal: 9, rawWithoutWhitespace: 8, bodyCount: 8,
                              step: "unicode_dirty", trace: trace)
            try storeWorkflowRequire(try disk(root, "main.tex") == Data(Self.initialMain.utf8), "Dirty edit saved implicitly")
        }
    }

    @MainActor @objc func testExplicitSaveThenUndoPreservesDiskAndBothCounts() {
        workflow { store, root, trace in
            let counts = StoreWorkflowCounts()
            try append("!", to: store)
            store.save()
            try storeWorkflowRequire(store.failure == nil && store.revision == 2 && !store.dirty && store.canUndo,
                        "Explicit Save changed history/revision or failed")
            try storeWorkflowRequire(try disk(root, "main.tex") == Data("alma fa!".utf8), "Save did not persist actual source")
            try counts.assert(store, rawTotal: 8, rawWithoutWhitespace: 7, bodyCount: 7,
                              step: "explicitly_saved", trace: trace)
            store.undo()
            try storeWorkflowRequire(store.failure == nil && store.revision == 3 && store.dirty && store.canRedo,
                        "Shared Undo after Save did not restore the genuine Store")
            try counts.assert(store, rawTotal: 7, rawWithoutWhitespace: 6, bodyCount: 6,
                              step: "undo_preserves_saved_disk", trace: trace)
            try storeWorkflowRequire(try disk(root, "main.tex") == Data("alma fa!".utf8), "Undo rewrote saved disk")
        }
    }

    @MainActor @objc func testLexicalMarkupDiffersFromRawSource() {
        workflow { store, root, trace in
            let counts = StoreWorkflowCounts()
            // Reviewed parser regressions remain substeps of this one workflow method.
            // Literal arithmetic is fixed before compilation; production counts never generate an oracle.
            typealias Row = (step: String, source: String, raw: Int, nonspace: Int, body: Int?,
                             status: LatexBodyCharacterCount.Status, issue: LatexBodyCharacterCount.Issue?)
            func grouped(_ levels: Int, _ body: String) -> String {
                String(repeating: "{", count: levels) + body + String(repeating: "}", count: levels)
            }
            let rows: [Row] = [
                ("lexical_markup_vs_complete_raw",
                 #"\textbf{alma fa} $x+y$ \label{k}"# + " % hidden\n", 42, 36, 6, .complete, nil),
                // a + two-backslash/star/[2pt] + b + space + backslash/linebreak/[4] + c:
                // 1+8+1+1+13+1=25 source characters; abc remains in the body.
                ("review_star_length_and_valid_strength",
                 #"a\\*[2pt]b \linebreak[4]c"#, 25, 24, 3, .complete, nil),
                ("review_unsupported_strength_withholds_count",
                 #"a\linebreak[5]b"#, 15, 15, nil, .unsupported, .bareSyntax),
                // A literal signed dimension is supported; no length macro expands.
                ("final_review_literal_fractional_length",
                 #"a\\*[-0.5em]b"#, 13, 13, 2, .complete, nil),
                ("final_review_invalid_length_withholds_count",
                 #"a\\*[nonsense]b"#, 15, 15, nil, .unsupported, .bareSyntax),
                ("final_review_dynamic_length_withholds_count",
                 #"a\\*[\input{other}]b"#, 20, 20, nil, .unsupported, .dynamicTeX),
                ("final_review_unknown_length_is_partial",
                 #"a\\*[\mystery]b"#, 15, 15, nil, .partial, .unknownCommand),
                // The opaque cite argument is a level: 2*63+8=134 succeeds; 2*64+8=136 refuses.
                ("review_opaque_combined_depth_64",
                 grouped(63, #"\cite{k}"#), 134, 134, 0, .complete, nil),
                ("review_opaque_combined_depth_65",
                 grouped(64, #"\cite{k}"#), 136, 136, nil, .unsupported, .nestingLimit),
                // "$ {x} $" is seven ASCII characters with two spaces and one math brace level.
                ("review_math_combined_depth_64",
                 grouped(63, "$ {x} $"), 133, 131, 0, .complete, nil),
                ("review_math_combined_depth_65",
                 grouped(64, "$ {x} $"), 135, 133, nil, .unsupported, .nestingLimit),
                // begin/equation (16) + x (1) + end/equation (14) =31;
                // closing the environment must not consume an additional nesting level.
                ("review_environment_closing_depth_64",
                 grouped(63, #"\begin{equation}x\end{equation}"#), 157, 157, 0, .complete, nil),
                ("review_environment_combined_depth_65",
                 grouped(64, #"\begin{equation}x\end{equation}"#), 159, 159, nil, .unsupported, .nestingLimit)
            ]
            for (index, row) in rows.enumerated() {
                try store.edit(range: NSRange(location: 0, length: store.text.utf16.count), replacement: row.source)
                try storeWorkflowRequire(store.failure == nil && store.revision == UInt64(index + 2) && store.dirty && store.canUndo,
                            "Actual Runtime markup edit/history failed at \(row.step)")
                let bytes = Data(store.text.utf8), revision = store.revision
                let selection = store.selection, undo = store.canUndo, redo = store.canRedo
                try storeWorkflowRequire(bytes == Data(row.source.utf8), "Runtime source bytes differ at \(row.step)")
                try counts.assert(store, rawTotal: row.raw, rawWithoutWhitespace: row.nonspace,
                                  bodyCount: row.body, bodyStatus: row.status, issue: row.issue,
                                  step: row.step, trace: trace)
                try storeWorkflowRequire(Data(store.text.utf8) == bytes && store.revision == revision &&
                            store.selection == selection && store.dirty && store.canUndo == undo && store.canRedo == redo,
                            "Count observation mutated the actual Store at \(row.step)")
                try storeWorkflowRequire(try disk(root, "main.tex") == Data(Self.initialMain.utf8),
                            "Markup observation saved implicitly at \(row.step)")
            }
        }
    }

    @MainActor @objc func testPreferenceHideShowPreservesStoreAndRawProjection() {
        workflow { store, root, trace in
            let counts = StoreWorkflowCounts()
            try append("!", to: store)
            let bytes = Data(store.text.utf8), revision = store.revision, selection = store.selection
            let dirty = store.dirty, undo = store.canUndo, redo = store.canRedo
            try storeWorkflowRequire(counts.preferences.characterCountMode == .source, "UI56 source default changed")
            try counts.assert(store, rawTotal: 8, rawWithoutWhitespace: 7, bodyCount: 7,
                              step: "visible_source_and_body_primitives", trace: trace)
            counts.preferences.characterCountMode = .latexBody
            counts.preferences.showSourceCharacterCount = false
            try storeWorkflowRequire(counts.raw(store) == nil, "Production raw projection ignored hide")
            let restored = EditorPreferences(storage: counts.memory)
            try storeWorkflowRequire(restored.characterCountMode == .latexBody && !restored.showSourceCharacterCount,
                        "Memory preference recreation lost mode/visibility")
            counts.preferences.showSourceCharacterCount = true
            counts.preferences.characterCountMode = .source
            let raw = counts.raw(store)
            try storeWorkflowRequire(raw?.includingWhitespace == 8 && raw?.excludingWhitespace == 7, "Production raw show failed")
            try store.refreshInfo()
            try storeWorkflowRequire(Data(store.text.utf8) == bytes && store.revision == revision && store.selection == selection &&
                        store.dirty == dirty && store.canUndo == undo && store.canRedo == redo,
                        "Preferences/projection mutated actual Runtime→Store state")
            try storeWorkflowRequire(counts.memory.writtenKeys == ["ide.characterCountMode", "ide.showSourceCharacterCount",
                        "ide.showSourceCharacterCount", "ide.characterCountMode"], "Unexpected preference writes")
            // No fake body-visibility adapter is substituted. The production body visibility guard is private SwiftUI;
            // this method covers real preferences/raw projection and genuine Store nonmutation, not rendered body text.
            trace.capture("visibility_scope_boundary", store, raw, counts.body(store))
            store.undo()
            try storeWorkflowRequire(store.failure == nil && store.canRedo, "Preference changes consumed actual Undo")
            try counts.assert(store, rawTotal: 7, rawWithoutWhitespace: 6, bodyCount: 6,
                              step: "preference_changes_preserved_shared_undo", trace: trace)
            try storeWorkflowRequire(try disk(root, "main.tex") == Data(Self.initialMain.utf8), "Toggle/Undo wrote disk")
        }
    }

    @MainActor @objc func testDocumentSwitchAndEmptyRuntimeBuffer() {
        workflow { store, root, trace in
            let counts = StoreWorkflowCounts()
            try append("!", to: store)
            store.select("second.tex")
            try storeWorkflowRequire(store.failure == nil && store.selection == "second.tex" && store.revision == 1 && !store.dirty,
                        "Second genuine Runtime document not loaded")
            try counts.assert(store, rawTotal: 4, rawWithoutWhitespace: 2, bodyCount: 2,
                              step: "second_loaded", trace: trace)
            try append("🇭🇺", to: store)
            try counts.assert(store, rawTotal: 5, rawWithoutWhitespace: 3, bodyCount: 3,
                              step: "second_dirty", trace: trace)
            store.select("empty.tex")
            try storeWorkflowRequire(store.failure == nil && store.selection == "empty.tex" && store.text.isEmpty && !store.dirty,
                        "Empty Runtime-backed document not loaded")
            try counts.assert(store, rawTotal: 0, rawWithoutWhitespace: 0, bodyCount: 0,
                              step: "empty_runtime_buffer", trace: trace)
            store.select("main.tex")
            try storeWorkflowRequire(store.failure == nil && store.dirty && store.revision == 2, "Main dirty state lost on switch")
            try counts.assert(store, rawTotal: 8, rawWithoutWhitespace: 7, bodyCount: 7,
                              step: "main_dirty_restored", trace: trace)
            try storeWorkflowRequire(try disk(root, "main.tex") == Data(Self.initialMain.utf8), "Switch saved main")
            try storeWorkflowRequire(try disk(root, "second.tex") == Data(Self.initialSecond.utf8), "Switch saved second")
            try storeWorkflowRequire(try disk(root, "empty.tex").isEmpty, "Switch changed empty disk fixture")
        }
    }

    @MainActor @objc func testStaleSyntaxAndCompositionTeardownRefusal() {
        workflow { store, root, trace in
            let counts = StoreWorkflowCounts(), stale = store.revision
            try append("!", to: store)
            let accepted = Data(store.text.utf8), revision = store.revision
            let range = NSRange(location: 0, length: store.text.utf16.count)
            try storeWorkflowRequire(store.syntax(document: "main.tex", revision: revision, range: range) != nil,
                        "Positive same-revision genuine Runtime syntax control failed")
            try storeWorkflowRequire(store.syntax(document: "main.tex", revision: stale, range: range) == nil,
                        "Stale Runtime request was accepted")
            try storeWorkflowRequire(store.syntax(document: "main.tex", revision: revision, range: range) != nil,
                        "Stale refusal destroyed the current Runtime handle")
            try storeWorkflowRequire(Data(store.text.utf8) == accepted && store.revision == revision && store.dirty,
                        "Stale observation mutated Store")
            try storeWorkflowRequire(store.beginComposition(), "Actual Runtime composition did not begin")
            try storeWorkflowRequire(!store.requestTermination() && store.isComposing && store.failure != nil,
                        "Composition teardown did not refuse before the modal-alert branch")
            trace.capture("composition_teardown_refused", store, counts.raw(store), counts.body(store))
            // End the genuine composition with a real edit; do not seed isComposing/revision/runtime.
            try store.edit(range: NSRange(location: store.text.utf16.count, length: 0), replacement: "?", composition: true)
            try storeWorkflowRequire(!store.isComposing && Data(store.text.utf8) == Data("alma fa!?".utf8), "Composition end failed")
            store.failure = nil // Clear only the expected refusal's presentation message.
            try counts.assert(store, rawTotal: 9, rawWithoutWhitespace: 8, bodyCount: 8,
                              step: "refused_teardown_kept_runtime_operable", trace: trace)
            try storeWorkflowRequire(try disk(root, "main.tex") == Data(Self.initialMain.utf8), "Refusal saved implicitly")
            // Never call requestTermination with composition=false: its error/dirty branch may create a modal alert.
            // Existing Store has no noninteractive explicit-close API. withStore releases this instance by ARC,
            // then checks its own descriptor cleanup; post-close calls on a live Store remain untested.
        }
    }
}
