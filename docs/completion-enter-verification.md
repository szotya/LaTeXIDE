# Completion Enter — native keyboard follow-up

2026-10-31. The user reported that Enter selected a candidate but left the chooser open, requiring Escape. During the investigation the user reported that it now appeared to work. The current implementation and running app accepted and closed the tested suggestions. The original stuck state was not reproduced, so its cause is not established and this follow-up contains no production-code fix.

## Verified behavior

Enter/Return accepts the selected current candidate and closes the popup in the same action. Source focus remains available; a subsequent Enter inserts an ordinary newline. Escape dismisses without accepting. Existing stale-proposal and marked-text guards remain covered by the earlier suites.

The new [CompletionKeyboardTests](../apps/macos/Tests/LaTeXIDEKitTests/CompletionKeyboardTests.swift) send real `NSEvent` key-down events through an `NSWindow` with the production source editor and actual Rust workspace. They retain the original popup and assert that it becomes invisible, is detached from the parent, and stays dismissed after the 80 ms completion / 100 ms coloring callbacks. Merely inserting the expected text is insufficient.

Five cases pass: main Return; numeric-keypad Enter; Down-selected project macro; an already complete reference with no acceptance edit/history; and a citation preserving another key. Each checks a second Enter, source focus and shared undo. These are characterization/ongoing regression checks: all passed against the existing product code on their first execution; no RED→GREEN repair is claimed.

## Fresh results

Evidence root: `evidence/completion-enter-2026-10-31/`.

| Evidence | Result |
| --- | --- |
| `keyboard-initial.log` | Initial real-window Return/keypad characterization: 2 PASS. |
| `keyboard-tests.log` | All 5 keyboard cases PASS. |
| `native-tests.log` | Full native suite: 62 executed, 58 PASS, 4 opt-in skipped, 0 failures. |
| `bundle-check.json`, `codesign.log` | Existing packaged app unchanged: all 4 artifact hashes match the prior manifest, no newer production inputs, strict local ad-hoc signature valid. No app rebuild was needed for test/documentation changes. |
| `live-before.json`, `live-restored.json`, `live-observations.json` | Live source/focus observations and scoped test-buffer restoration; user edits and all disk hashes preserved. |
| `skill-validation.log`, `markdown-links.json` | Formal skill valid; changed-document/context scope: 9 Markdown files, 54 relative targets, 0 broken. A separate whole-tree snapshot preserves an absent main-report link in an unrelated, externally added audit backlog; those audit files were left unchanged. |

Commands from the repository root:

```sh
CLANG_MODULE_CACHE_PATH="$PWD/.local/clang-module-cache" swift test --package-path apps/macos --scratch-path "$PWD/.local/swift-build" --cache-path "$PWD/.local/swift-cache" --config-path "$PWD/.local/swift-config" --security-path "$PWD/.local/swift-security" --disable-sandbox --filter CompletionKeyboardTests
CLANG_MODULE_CACHE_PATH="$PWD/.local/clang-module-cache" swift test --package-path apps/macos --scratch-path "$PWD/.local/swift-build" --cache-path "$PWD/.local/swift-cache" --config-path "$PWD/.local/swift-config" --security-path "$PWD/.local/swift-security" --disable-sandbox
codesign --verify --deep --strict --verbose=2 dist/LaTeXIDE.app
```

The native test invocation needs the existing narrow Xcode/local-test-runtime sandbox permission described in the handoff. No model request or changed compiler/shared Rust code was involved, so their earlier results were not rerun or relabeled as new results.

## Live observation and scope

In the already running project-completion sample, CUA Return changed `\sec` to `\section{}`; reference completion changed `\ref{sec:}` to `\ref{sec:introduction}`, and the next Return inserted a newline. AX retained source focus. Live screenshots remained tiny/distorted, so precise popup visibility proof comes from the controlled native-window assertions rather than readable screenshots.

The user continued editing `main.tex` during this check. Global undo was therefore stopped; the two known test buffers were restored separately through expected-revision edits only when their text/revision matched the test. The user main buffer stayed r6/dirty with identical contents across restoration, all disk hashes stayed unchanged, and the app remained open. Its current dirty user work is not the prior clean fixture baseline.

The [previous project-completion report](project-completion-verification.md) and its app manifest remain historical feature/build evidence; the same bundle is still current. Windows/Linux native keyboard behavior remains unrun. Port the [popup closure contract](../skills/latexide-platform-development/references/project-completion.md) with actual target controls and events.
