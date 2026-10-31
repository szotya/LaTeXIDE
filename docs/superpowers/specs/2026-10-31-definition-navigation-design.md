# Definition navigation design

The user requested Cmd+click navigation for own macros, ref labels and cite BibTeX keys, with continued Markdown/skill maintenance. This extends the accepted native/Rust architecture and existing lexical completion index. Implement locally in the current checkout under the existing execution authorization; no Git initialization, commit, publication or extra approval round.

## Approach and behavior

Extend the existing bounded project index with exact declaration ranges, then expose a revision-bound `get_definition` query. This keeps completion/navigation precedence, dirty-buffer overlay, supported declaration/key families and project confinement consistent. A separate Swift parser would duplicate these rules; a full language-server migration would broaden this request substantially. The shared-index choice is the implementer's routine integration decision.

Cmd+click on a macro control sequence (including its backslash), a ref-family key or an individual cite-family list key opens the defining file, selects the declaration name/key and scrolls it into view. Keep source focus. Plain clicks and extra modifier combinations retain normal native behavior. Ignore prose, delimiters, comments/verbatim, unresolved or built-in-only names and active marked-text composition. Navigation never edits/saves text or adds undo history. Existing tab selection/viewport state survives.

Use the existing deterministic lexical winner: active source file, other open files sorted, then other files sorted; last declaration within a file. Report multiple indexed matches and partial coverage in compact navigation feedback. This is not TeX include/scope resolution or an ambiguity chooser; a semantic graph is future work.

## Freshness and coordinates

Store declaration start/length in scalar-safe absolute UTF-16, plus its actual one-based line. Disk indexing must normalize BOM/CRLF through the same document decoder as opening a file. Compute offsets with a monotonic cursor, avoiding repeated prefix scans for many symbols. Preserve current 128-file / 8 MiB / 1 MiB per file / 8192-symbol / 10000-entry index limits and 10-open-document limit.

`Workspace.definition(id, expected_revision, offset_utf16)` / `get_definition` resolves the exact full symbol at a clicked character, not a completion prefix. It forces unopened disk refresh, even with identical size/mtime. Queries do not open buffers or change revisions/history. Reply includes source range, symbol/kind, one target with relative document id, optional open revision, exact range/line, alternative count and optional index status.

Native navigation reads/opens the target without switching visible selection, repeats the query against the original source/revision, checks target identity/revision/range/selected text, then loads that expected target revision and publishes revision-bound range navigation. Changed/removed/unreadable/over-limit targets fail without a wrong selection or user edit. Recheck revision before applying the native selection. This handles open dirty targets and disk changes between query and open.

## Verification and platforms

Real Rust/filesystem tests cover exact macro/key hits, lists/options, Unicode, ignored contexts, lexical precedence, dirty edit/undo, forced disk refresh, normalized target offsets, bounds and nonmutation. Actual IPC verifies additive protocol behavior. Native tests use real editor/layout coordinates and Cmd mouse events for all three kinds, target selection/scroll/focus, dirty target and history; plain/extra modifiers, whitespace, IME and stale revisions must not trigger navigation. Build/sign the arm64 app and check shared MSVC x64 types. Preserve the running user's dirty work; use isolated native test fixtures instead of altering/restarting that session.

Windows/Linux consume the shared query and must implement their own native gesture, exact selection and target freshness checks. No native target-platform run is available here. No new dependency, hover query loop, back/forward history, rename, usage index or compiler/model call is part of this increment.
