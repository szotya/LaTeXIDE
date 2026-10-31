# Project macros, references and BibTeX completion

Read `<repo>/docs/project-completion-verification.md` and its linked design/review before extending the feature. Shared Rust source is `crates/latexide-core/src/{completion,tex_scan,project_index,workspace}.rs`; Mac consumers are WorkspaceStore, WorkspacePresentation and Editor/CompletionController. No new runtime dependency/language server is required.

## Authoritative project data

Index `.tex/.ltx/.sty/.cls/.bib` under the project root. Open Rust documents override their disk copies, including dirty agent edits and undo. Indexing must not open documents/tabs, consume the ten-document budget, save or mutate revisions/history. Deterministic precedence: active file → other open files sorted → remaining files sorted; last declaration within a file. This is lexical precedence, not TeX include order or arbitrary expansion.

Bounds:128files,8MiB total,1MiB/file,8192symbols,10000visited entries,20candidates. Skip symlinks/hidden/artifact directories and reject escaped canonical roots. Read source boundedly; invalid/unreadable/over-budget data yields partial coverage. Keep retained path strings shared rather than copying them for every symbol. Do not serialize full source on each query.

Disk discovery/metadata cache is1s; dirty open revisions bypass it immediately. Forced acceptance must re-read unopened content even with unchanged size/mtime. Already-open text follows the existing authoritative/external-change contract; do not silently overwrite it from disk. Document watcher/include-graph/LSP behavior is future work.

## Query and acceptance

Existing `get_completions {document_id,expected_revision,caret_utf16}` adds optional `refresh_project:false`. Response keeps absolute scalar-safe UTF-16 replacement range and insertion-relative selection. Items add `kind:command|macro|reference|citation`; optional `index_status {indexed_files,truncated,skipped_files}` reports coverage. Old consumers may ignore additive fields; updated native clients must use freshness semantics.

Before accepting, repeat the query with `refresh_project:true` and verify the selected range, label/kind, insertion and selection still match. A different file can change without changing the active revision. Stale Enter/Tab dismisses and falls through to ordinary editing. If insertion already equals source, consume acceptance and move the caret without adding an edit/history entry or literal tab. Remember the accepted document/revision/caret so pending callbacks do not immediately reopen the same popup; a changed context can query again. Otherwise insert once through the existing native/core adapter, preserving shared undo/IME/focus rules.

Enter/Return (including numeric-keypad Enter) must both accept and close the chooser in one action, keeping source focus. Escape closes without accepting. Verify actual native events and popup visibility/parent detachment after delayed refreshes, not just inserted text or a direct controller call. Include arrow-selected macros, already-complete/no-edit keys, citation lists, ordinary second Enter and shared undo. Mac characterization is `CompletionKeyboardTests`; latest evidence is `<repo>/docs/completion-enter-verification.md`. A currently passing characterization does not establish the cause of an intermittent user report or a new fix.

Decode/show index status even when there are zero items. Mac keeps a compact popup type badge/provenance and a status-bar partial hint; do not claim complete project coverage after a budget/read failure. This is actual index state, not a simulated progress signal.

## Lexical subset and failure recovery

Own macro families: new/renew/providecommand, DeclareRobustCommand, DeclareMathOperator, def/gdef/edef/xdef and New/Renew/Provide/DeclareDocumentCommand. Simple plain parameters and common `m/o/O/s` signatures get required braces; unsupported/delimited signatures insert bare names. Preserve existing stars/arguments. Control words are ASCII letters/`@`, max64bytes; keys max256bytes, Unicode alphanumeric plus `_:-./+!?@`.

Reference/citation command families and exclusions are enumerated in verification. Replace the whole current comma-separated key, including suffix after caret; preserve whitespace/other keys and omit keys already present elsewhere. A whitespace-only segment is a zero-length key at the caret. Repair a missing closing brace only on a final list segment; leave nonfinal unfinished lists intact. Do not insert an early `}` that ejects later keys.

Skip TeX comments/verb/verbatim-like/comment environments and BibTeX comment/string/preamble pseudoentries. Respect nested braced/quoted BibTeX fields. Context groups are limited to16KiB; unverified longer groups suppress suggestions. Short literal-name reads and aggregate signature lookahead budgets are essential: repeated `\begin{` once froze a near-limit query for seconds despite a per-group16KiB cap. A per-call bound multiplied by many commands is not an aggregate bound.

## Verification and ports

Exact macro/ref/cite navigation reuses this same index and precedence. See [definition-navigation.md](definition-navigation.md) for clicked-character ownership, normalized declaration ranges, forced read-only resolution and native target freshness. A completion prefix is not itself an exact definition hit.

Use real filesystem/workspace tests plus actual IPC and native controls. Keep malformed near-limit latency regression, ordinary cold/warm corpus timing, dirty cross-file undo, unchanged-mtime forced refresh, list/scalar/empty contexts, limits and nonmutation coverage. Core-only dev `opt-level=2` was chosen after measured cold cost; it retains symbols/assertions but can affect stepping. Do not call these pure-core timings release/end-to-end/RAM proof.

Windows/Linux must implement native popup/selection/fresh acceptance/field-local undo/status with their APIs. MSVC Rust check is not native execution. See `windows.md`/`linux.md`; preserve their unrun boundaries. Update the central handoff/ledger and this reference when changing a reusable parser/cache/wire/port contract; keep detailed chronological evidence in Markdown rather than here.
