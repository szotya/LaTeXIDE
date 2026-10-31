# Definition navigation — implementation and verification

2026-10-31. User request: Cmd+click own macros, ref labels and cite BibTeX keys; maintain Markdown and the developer skill. [Design](superpowers/specs/2026-10-31-definition-navigation-design.md), [implementation plan](superpowers/plans/2026-10-31-definition-navigation.md). Implemented and verified on Mac native controls, one integrated review and one reproducing-test fix pass complete. Initial and final results remain separate below.

## Implemented behavior

Cmd+click in the native Mac source opens the project's defining file, selects the exact macro name (including backslash), label key or BibTeX entry key, and scrolls it into view. Ref/cite lists and optional arguments are understood; clicking another part of the list resolves that particular whole key. Dirty authoritative buffers and undo determine targets. Navigation changes no source text, save state or edit history; source selection/viewport survive tab return.

Plain clicks/extra Shift/Option/Control modifiers, blank space outside a glyph, marked/composing input and stale rendered text do not invoke the resolver. Missing, changed or over-limit targets give compact nonmodal feedback. The existing ten-open-buffer limit still applies when actually opening a target. A query alone never opens buffers. Old line-based diagnostic/go-to-line navigation remains supported.

Shared `get_definition {document_id,expected_revision,offset_utf16}` resolves exact full tokens through the existing bounded project index. It forces unopened disk content refresh, including preserved size/mtime. Index coordinates use the same BOM/CRLF decoder as opening a file and a monotonic UTF-16/line cursor. Native resolve/read/re-resolve and expected-revision loading validate target identity, revision, scalar range and exact text before selection; native range requests also carry the target revision.

Resolution retains completion's lexical precedence: active source file, other open files sorted, remaining files sorted; last declaration per file. Multiple indexed matches and partial coverage are reported. This is not TeX include/scope expansion, a complete language server or a multiple-definition picker. Supported macro/ref/cite families and project limits remain those in the [completion report](project-completion-verification.md).

## Initial implementation evidence

Evidence root: `evidence/definition-navigation-2026-10-31/`.

| File | Observed result |
| --- | --- |
| `core-red.log` | Missing shared API/types before implementation. |
| `rust-tests.log` | Full Rust workspace 67 PASS: 11 new definition contracts plus previous 56. Exact ranges, Unicode/multiline declarations, lists/options, ignored/delimiter/prefix contexts, dirty edit/undo, precedence, unchanged-metadata refresh, BOM/CRLF, query nonmutation, bounds and symlink confinement. |
| `ipc-red.log`, `ipc-tests.log` | Actual unknown-operation RED, then full 10 socket contracts PASS. New case checks dirty target revision/range, nonmutation, stale source and undo. |
| `native-red.log` | Missing native members before implementation. |
| `native-first-integration.log` | Hidden-window synthetic dispatch did not route mouse-down; the test boundary was corrected to the actual production source mouseDown with the same NSEvent/coordinates. No product workaround. |
| `native-green.log`, `native-tests.log` | Initial 8-case targeted PASS, then 9 new native cases in the full suite: 71 executed / 67 PASS / 4 opt-in skipped / 0 failures. Includes all three kinds, focus/source state, dirty undo, normalized target, far-down scroll, modifier/blank/IME/stale guards, missing and document-limit handling. |
| `macos-build.log` | Staged arm64 app build/signature exit0; old mapped app inode retained. |
| `windows-rust-check.log` | All five shared crates MSVC x64 type-check PASS; not a Windows native app build/run. |
| `mcp-tests.log` | Existing stdio bridge roundtrip 1 PASS. Nine-tool assistant policy unchanged; no model request. |

One implementation test exposed multiline BibTeX keys after an entry opener: the parser previously stopped at the newline. Whitespace after the opener now permits that key, with its actual line/range. Disk symbols now reject the same invalid UTF-8/NUL/mixed-line-ending sources as the editor, marking partial coverage.

## Review fixes and final results

One fresh read-only review reported two Important issues and a duplicate-BibTeX precedence issue initially graded Minor. The author regraded the latter for consistency during editing. All three reproduced before fixes: final macro letter adjoining a comment/verbatim token; old navigation replay after last-tab editor recreation; first-versus-last duplicate BibTeX winner. [Findings, author decisions and remaining limits](definition-navigation-review.md). No re-review or deferred minor remains.

Character ownership now stays on the clicked character (only a leading backslash advances into command context); completion caret semantics are unchanged. TeX/BibTeX share last-per-file normalization. Navigation consumption now belongs to persistent nonpublished WorkspaceStore state, preventing old requests from overriding a later user caret/viewport after editor recreation. Stored target revisions still guard stale selection.

| Final evidence | Observed result |
| --- | --- |
| `review-core-red.log`, `review-native-red.log` | Both shared regressions fail as expected; native recreation fails with old target selection/scroll overriding caret0/scroll0. |
| `review-native-green.log` | Recreation regression PASS after persistent consumption. |
| `rust-tests-final.log` | Full Rust workspace **69 PASS**, including **13 definition contracts**. |
| `native-tests-final.log` | **73 executed / 69 PASS / 4 opt-in skipped / 0 failures**, including **11 definition cases** and the five earlier Enter cases. The final-letter-before-comment Cmd event also passes with the final linked core. |
| `ipc-tests-final.log`, `mcp-tests-final.log` | Fresh IPC **10 PASS**, existing MCP stdio **1 PASS**. |
| `windows-rust-check-final.log` | Fresh all-five-crate shared MSVC x64 type-check PASS. |
| `macos-build-final.log`, `macos-app-manifest.json`, `codesign.log` | Final staged arm64 app build exit0; four packaged artifacts hashed, canonical palette/static-core stamp match, no newer production inputs, strict local ad-hoc signature valid. Not notarization. |

The four default native skips remain the installed-CLI handshake and previous opt-in render cases; none was relabeled as run for this feature. No new compiler run, model request, release performance/RAM result or live packaged-app Cmd-click is inferred. Bundle verification is reproducible with `python3 evidence/definition-navigation-2026-10-31/check_bundle.py`.

Documentation closure: formal project-skill validator returned `Skill is valid!`; all current root/docs/project-skill Markdown links checked: **42 documents / 283 relative targets / 0 broken** (external URLs, anchors and fenced examples excluded). Reports: `skill-validation.log`, `markdown-links.json`, `verification-summary.json`. Existing date-stamped audit artifacts and previous phase manifests remain intact. No Git commit/publication, global skill/memory update or user-session restart was performed.

## Reproduce and maintain

Use the canonical handoff commands from the project root. Targeted core: `bash scripts/cargo-local.sh test -p latexide-core --test definition_contract --offline`; native: canonical Swift test command with `--filter DefinitionNavigationTests`. Full checks: workspace Cargo tests, full Swift tests, `python3 tests/ipc_contract.py`, `python3 tests/mcp_contract.py`, `bash scripts/build_macos.sh`, shared MSVC x64 check. No dependency download is needed.

The native tests create their own off-screen NSWindow, real source/layout and workspace, send NSEvent mouse-down to SourceTextView, and assert state/selection/scroll. This is a native-control event test, not physical mouse/window-server or newly launched packaged-app proof. The user's running app and dirty work were preserved; no live new-bundle Cmd-click is claimed. The old process needs a normal save/restart to load the newly staged app.

Reusable native/wire/port instructions: [definition navigation skill reference](../skills/latexide-platform-development/references/definition-navigation.md). Windows/Linux native gestures and execution remain unrun. Back/forward history, ambiguity picker, expanded scope, hover/provider UI, rename and usage navigation remain future capabilities. The independent date-stamped IDE audit retains its own earlier snapshot/test boundary.
