# Project-aware completion — Mac verification

Updated 2026-10-31. Current increment after [keyboard/basic completion/Codex integration](intelligence-verification.md). User request: own macros, ref labels and BibTeX keys, with maintained Markdown and developer skill.

Later follow-up: [Enter accepts and closes the actual popup](completion-enter-verification.md), with five additional native keyboard tests and a fresh full-suite result. The feature/build results below retain their original scope.

Subsequent feature/build: [exact definition navigation](definition-navigation-verification.md), with Cmd+click and the latest package manifest in that report. The original completion-phase counts/manifests below remain historical.

## Implemented workflow

The shared Rust index reads project-local `.tex`, `.ltx`, `.sty`, `.cls`, `.bib` and feeds the existing native candidate popup. Each result carries a type badge and definition file/line. Open in-memory documents, including dirty agent edits and undo, override their disk copies. Indexing does not create editor documents, save or mutate source/history.

Own macro families: new/renew/providecommand, DeclareRobustCommand, DeclareMathOperator, def/gdef/edef/xdef and New/Renew/Provide/DeclareDocumentCommand. Simple required arguments get braces/first-argument caret; optional/default arguments are omitted. Common `m/o/O/s` specs and modifiers are recognized; unknown/delimited signatures insert the bare name. Existing arguments/stars survive.

Reference families: ref/eqref/pageref/autoref/nameref/vref/cref/Cref. Citation families: cite/citep/citet/Citep/Citet/autocite/parencite/textcite/footcite/smartcite/supercite/nocite. Prefix matching is case-sensitive. Lists replace only the current key, preserve optional arguments/whitespace/other keys and exclude already-used keys. Whitespace-only segments work. A missing closing brace is repaired only for the final segment; an earlier segment leaves the surrounding unfinished list intact.

BibTeX brace/parenthesis entries, nested braced/quoted fields and Unicode keys are recognized; comment/string/preamble pseudoentries are excluded. TeX comments, verb/Verbatim/verbatim/lstlisting/minted/comment environments are ignored. Static key syntax is conservative; dynamic expansion, include order, conditionals/catcodes, installed package metadata, custom citation command semantics and full LSP services are not implemented.

## Bounds and freshness

- 128 indexed files,8MiB aggregate source,1MiB/file,8192 retained symbols,10000 visited directory entries,20 candidates. Hidden paths/artifact directories and symlinks are skipped. Unreadable/invalid/over-budget sources produce partial coverage rather than an edit error. The popup and status bar expose partial state, including zero matching candidates.
- Disk discovery/metadata is cached for up to1s during typing; open revisions are checked immediately. Acceptance forces a bounded content re-read for unopened files, even with unchanged size/mtime, and validates the chosen range/insertion/selection. Stale Return/Tab falls through; an already complete current key is accepted without a source edit or stray tab.
- Macro names: ASCII control words/`@`, maximum64bytes. Keys: up to256bytes, Unicode alphanumeric and `_:-./+!?@`. Argument context lookahead16KiB; unverified longer groups do not produce candidates. Signatures have bounded short reads and an aggregate work budget; truncated metadata remains partial. All wire positions are scalar-safe UTF-16.
- Precedence: active file, other authoritative open files sorted, remaining project files sorted; last declaration within a file wins. This deterministic lexical rule is not a compiler scope/expansion claim. Open documents retain the existing external-change/reload boundary.

## Automated results

Paths below are relative to `evidence/project-completion-2026-10-31/`.

| Evidence | Actual result |
| --- | --- |
| `core-red.log` |12 initial missing-feature failures against the previous implementation. Fixtures were corrected before product code for independently valid prefixes/caret positions. |
| `rust-tests.log` |56PASS:13document,8syntax,6basic completion,21project completion,8build. Real filesystem tests cover unopened files, dirty cross-file edit/undo, Unicode/full-key replacement, lists/options/duplicates, malformed input, source/symbol/file bounds, symlink confinement and no editor/history mutation. |
| `context-red.log` |Reproduced false comment-environment symbols and a false closing brace at a truncated context boundary; both fixed and covered in final suite. |
| `ipc-red.log` / `ipc-tests.log` |Unchanged size/mtime forced-refresh test failed first with `sec:old`; final real native-library/socket suite9/9PASS, including dirty agent changes in another document and unchanged query revision. |
| `native-red.log` |Actual AppKit stale-other-buffer and stale-disk candidate acceptance failures before fresh native validation. Initial test execution was interrupted because a failed test's dirty cleanup opened a modal termination dialog; test-only cleanup now saves only its own temporary fixture. This interruption is not a PASS. |
| `native-tests.log` |57executed,53PASS,4opt-in skipped,0failures. New actual AppKit cases cover macro/ref/cite insertion/caret/one undo, cross-file stale Enter/Tab, partial state with zero matches, comma-space lists, unfinished lists and exact-key no-op acceptance and stable dismissal until context changes; previous focus/IME/history/assistant regressions also pass. One intermediate partial-status test used the wrong caret after the closing brace; it was corrected to the fixture's actual marked selection, without a product change. |
| `review-red.log`, `review-native-red.log`, `exact-key-red.log`, `accepted-popup-red.log` |Material review and exact-key reproductions; fixes and dispositions in [review record](project-completion-review.md). |
| `native-render.log` / `project-*-dark.png` |1 opt-in method PASS; three actual Rust-backed native popup renders (macro/reference/citation). `render-scope.txt`: off-screen components, not live screenshots or mocked candidates. |
| `mcp-tests.log`, `compiler-tests.log` |Real stdio bridge1/1 and TeX Live2026/latexmk4.88/XeLaTeX runtime3/3PASS. |
| `windows-rust-check.log` |All five crates MSVC x64 type-check PASS; native Windows/Linux UI execution remains unrun. |
| `macos-build.log` |Final staged debug arm64 app rebuilt after the fix pass, bundled compiler/bridge/palettes, strict local ad-hoc signing. Manifest/skill/link closure is recorded below. |

## Measured core query cost

Same own101-file corpus, approximately3.8MB source. Initial unoptimized debug cold208–225ms, warm p95~0.17ms. Core-only dev `opt-level=2` retains symbols/assertions; final recorded cold17.854ms, warm p50.028ms/p95.033ms. These are Rust query timings, not whole keystroke/debounce/native rendering latency or release/RAM qualification.

The review's malformed near-limit reproducer blocked for seconds. Own900KB repeated-begin case:5.398s RED →43.242ms GREEN. Other final malformed shapes: newcommand5.204ms, optional-count3.779ms, document-command7.561ms, def5.665ms. Five inputs have a generous1s synchronous regression guard. Retain those numbers with their input/profile; do not generalize them into a platform performance guarantee.

## Actual live app

The previous running sample had user-origin saved revision25. Its source and the original `evidence/mac-gui-project/main.tex` were hash-recorded and preserved, then the app quit normally. A new separate own `demo-project` under this evidence root was launched through the existing `--project` argument. No forced termination or global preference/model/auth change.

- Typed `\my`, selected the second own macro with Down+Tab → `\mynorm{}` r4, caret inside argument. One actual logical Cmd+Z restored `\my` r5; Shift+Cmd+Z restored the insertion r6. Further undo restored the original single backslash r9/clean. Typing the two characters had separate native transactions; the candidate insertion itself was one transaction.
- In `\ref{sec:}`, Down+Tab inserted `sec:methods` defined in `chapters/methods.tex`, r2; one undo restored r3/clean.
- In `\cite{knuth1984, }`, Tab inserted the unused `lamport1994`, preserving the existing key/comma/space/brace, r2; one undo restored r3/clean.
- Same-session protected IPC then changed the other file's label to `sec:methodology`, origin agent/r2/dirty, without saving or changing the reference document's r3. The native query immediately offered the dirty definition; Down+Tab inserted it, reference r4. Two actual Cmd+Z actions restored the insertion and the agent's definition change. This live edit used protected IPC; the separate real stdio MCP suite remains distinct. No new model call was needed.
- Toolbar Build succeeded/nonstale,11889-byte PDF. Live PDFKit AX showed the title, resolved reference and `E[X]`. All five retained documents were clean at the end; both previously existing user sample hashes matched. Exact assertions: `live-*.json`, `live-agent-definition.json`, `live-build.json`, `preserved-inputs.json`.

CUA initially failed with ScreenCaptureKit -3811; after normal restart/reset AX/input worked. Captures remained tiny/distorted, so readable live screenshot acceptance is **not** claimed for this increment. The separately inspected controlled popup PNGs and real AX/input/IPC evidence are different proofs. Existing compiler diagnostics retained three initial-pass reference/rerun warnings although latexmk resolved the reference in the final PDF; final-pass diagnostic filtering is a future compiler/UI quality task, not changed here.

## Handoff and remaining scope

Current source: `completion.rs`, `tex_scan.rs`, `project_index.rs`, Workspace/protocol/FFI; native WorkspaceStore/DTO/controller/status bar and project completion tests. Reproduction uses the existing handoff's cargo/Swift/cache commands; targeted suites are `--test project_completion_contract` and `--filter ProjectCompletionTests`. Render: `LATEXIDE_RENDER_PREVIEWS=1` with `--filter ProjectCompletionRenderTests`. No dependency download is required for the feature.

Reusable port guidance: [project completion skill reference](../skills/latexide-platform-development/references/project-completion.md). Windows/Linux need real native acceptance/focus/IME/status tests, not only the shared query. Full semantic project graph, definition navigation/refactoring, bibliography resolution, outline/SyncTeX/Git, document lifecycle/watch/recovery and release/accessibility/corpus qualification remain tracked in the product direction.


## Final package and documentation closure

After the accepted-popup fix, the final staged bundle was restarted normally and the comma-space citation acceptance was repeated. Tab inserted lamport1994, then Down moved the source caret from column29 to30 instead of navigating a reopened popup. Undo restored the fixture; fresh toolbar Build/PDFKit succeeded again. Final package session has main r1 and citation example r3, both clean;11901-byte nonstale PDF. `live-final-package.json` and `live-build-final-package.json` preserve this later smoke separately from the earlier five-document full workflow.

`macos-app-manifest.json`:four packaged artifact hashes, canonical palette match, Rust archive/relink stamp match, no newer native/shared build input. `codesign.log`:strict ad-hoc verification exit0; this is not notarization. `skill-validation.log`:Skill is valid. `markdown-links.json`:33Markdown documents/92relative targets,0broken (web URLs/anchors excluded). Current handoff/ledger/README/product matrix and project skill were updated; no Git commit, upload, global skill/memory or model request was added.
