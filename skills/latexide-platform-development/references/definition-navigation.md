# Exact definition navigation

Read `<repo>/docs/definition-navigation-verification.md` and its linked spec/plan. Rust implementation: `latexide-core/src/definition.rs`, ProjectIndex and Workspace; native Mac adapter: `Workspace/DefinitionNavigation.swift`, WorkspaceStore, NativeEditor and SourceTextView. Reuse the authoritative project index, never a second native-language parser/model.

## Query contract

`get_definition {document_id,expected_revision,offset_utf16}` is a read-only additive protocol-v1 operation, available through C dispatch/private IPC. It does not open editor buffers, edit/save text or change undo/redo. The native gesture is not a new assistant tool; keep the existing assistant allowlist unchanged unless deliberately extending that separate feature.

Offset addresses an actual character in source, with scalar-safe absolute UTF-16 coordinates. Resolve the exact whole macro control word (backslash included), ref-family key or individual cite-family list key, including options/stars. Avoid prefixes, punctuation/whitespace, prose, comments/verbatim and built-in-only commands. Keep clicked-character ownership distinct from a completion caret at the following boundary: a comment immediately after the token must not steal the last clicked character.

Reply: source `document_id/revision`, optional `symbol/kind/target`, `source_start_utf16/source_length_utf16`, `alternatives`, optional `index_status`. Target has relative `document_id`, nullable open-buffer `revision`, absolute `start_utf16/length_utf16` selecting the declaration name/key, and its one-based actual `line`. Macro symbol/selection includes the leading backslash. A missing target is a normal unresolved result; partial coverage remains explicit.

Deterministic lexical winner matches completion: active source → other open files sorted → remaining files sorted; last declaration in a file. Alternatives count indexed file winners. This is not expanded TeX scope/include order; surface multiple/partial matches and retain the limitation in documentation. A future semantic resolver/ambiguity chooser is separate work.

## Coordinates and freshness

Index exact declaration ranges using a monotonic UTF-16/line cursor; repeated whole-prefix scans per symbol can become quadratic. Disk index text must use the same Document BOM/CRLF decoder as opening a target. Invalid or unsupported sources count as partial rather than yielding offsets into text that cannot open. Preserve existing index bounds (128 files, 8 MiB, 1 MiB/file, 8192 symbols, 10000 visited entries) and the 10-open-buffer limit.

Definition lookup forces unopened-file content/discovery refresh, even with unchanged size/mtime. Open authoritative buffers override disk, including dirty agent edits and undo. On the native side: query using original source id/revision → read/open target without changing visible selection → query again against original source/revision → verify same target identity, target snapshot revision, scalar-safe range and exact selected symbol → load that expected revision → publish revision-bound native range navigation. Do not navigate a stale offset after a definition moved/changed/vanished. Actual navigation can open one target; query alone must not consume editor slots.

## Native event and presentation boundary

Mac handles left mouse-down with exact Cmd (ignoring benign device flags, excluding Shift/Option/Control). Hit-test a real laid-out glyph rectangle, accounting for text-container origin and view/window coordinates; nearest-insertion-index alone incorrectly links blank space after a line. Guard marked text/composition, rendered/source revision, document/project identity and authoritative text. Dismiss old completion before navigation, keep source focus and scroll/select the exact target.

`EditorNavigation` retains existing line-only navigation and adds optional range/revision. Check target revision before applying a stored range. Preserve source selection/viewport through the normal tab-state owner, and leave source edits/undo/save untouched. Use compact nonmodal feedback for missing/changed/over-limit targets and partial/multiple results; long paths truncate with full tooltip.

Consume each navigation serial in WorkspaceStore's persistent, nonpublished presentation state (`takeNavigation`), not only a disposable coordinator. Otherwise hiding every tab/recreating the editor can replay the old jump over a later user caret/viewport, even when the target revision is unchanged. A stale revision also consumes the request without applying its range. TeX and BibTeX must share last-per-file deduplication so duplicate dirty declarations have the same winner/count policy.

## Verification and ports

Use real Rust workspaces/files and actual IPC, plus native source controls receiving Cmd mouse events at glyph-derived coordinates. Cover all three symbol kinds, Unicode/BOM/CRLF, dirty moved target and undo, full-key/list/options, invalid positions, ignored/delimiter hits, forced disk refresh, nonmutation and budget confinement. Native cases must check exact target selection, far-away scroll visibility, source focus, original tab state, ordinary/extra-modifier and blank-space clicks, marked/stale state and document-limit failure.

An invisible off-screen NSWindow may not route a synthetic mouse event via sendEvent; use the production SourceTextView.mouseDown event boundary for isolated control tests and state the scope. Such tests are not global hardware/window-server/live-app proof. Keep the user's running dirty session intact during staged builds. Test fixtures own their queued mouse-up/cleanup; no test-only hooks belong in production classes.

Windows/Linux should use their native convention (typically Ctrl+click), glyph hit testing, authoritative target validation and exact range selection. Mac tests and shared MSVC type-check do not qualify a target-native implementation. Linux runtime-directory/UI work remains separate. Update handoff/ledger and this reference when changing the query/freshness/navigation contract.
