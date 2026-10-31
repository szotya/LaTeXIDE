# Native keyboard, completion and Codex assistant — verification

Updated2026-10-31. This is the current increment after the [developer UI phase](ui-verification.md). Product direction is maintained in [IDE_PRODUCT_DIRECTION.md](IDE_PRODUCT_DIRECTION.md); this is a tested Mac development implementation, not a completed multiplatform student release.

**Newer increment:** [project-aware macros, ref labels and BibTeX completion](project-completion-verification.md). Its current results supersede the original catalog-only/future-project-completion scope below. This document preserves the keyboard/Codex phase's historical observations.

## Implemented behavior

- Source undo/redo uses the shared Rust history, including agent edits after save. A focused assistant/search field owns its own native undo. Logical Cmd+Z/Shift+Cmd+Z are accepted without remapping a US physical key or consuming Option/Control combinations.
- While typing a LaTeX command, a themed native candidate popup appears. Up/Down selects, Tab/Enter inserts, Escape closes. The first argument is selected or receives the caret. An insertion is one shared undo transaction. Existing arguments/stars survive; comments/verbatim/marked text and ordinary prose do not trigger candidates. Completion can be disabled persistently.
- The native right inspector switches between PDF and Codex assistant. Panel dimensions, inspector mode, completion setting and existing themes/font preference persist. Unsent draft/attachments survive PDF/hide roundtrips within the workspace window. A new conversation/project reset intentionally drops old context; chat history is not persisted across restart.
- Codex CLI is an actual bounded subprocess using app-server JSONL stdio. Connect performs initialization only. Send creates an ephemeral thread, inherits the configured model/provider, streams real messages/tool status and supports cancellation. Selected source/diagnostics can be attached explicitly. Project change/window teardown terminates or disconnects the session and rejects stale events.
- Document operations go through the protected existing IDE MCP session. The transient config disables inherited MCP servers/apps/shell/web, grants only nine known IDE tools, and verifies read-only raw filesystem policy before a turn. Save is a separate operation and requires an explicit user request in the assistant instructions. There is no global config/auth change. Local CLI does not imply local/offline model inference.

## Automated evidence

Evidence paths below are relative to `evidence/intelligence-2026-10-31/`, except the keyboard rows.

| Check | Actual result and scope |
| --- | --- |
| Shared Rust `rust-tests.log` |35 pass:13document,8syntax,6completion,8build-supervisor. Completion covers whole-command replacement, bounded candidates, Unicode/scalar caret, comments/verbatim/escaped slashes, existing arguments and starred commands. |
| Native `native-tests.log` |46 executed,43 passed,3 opt-in skipped,0 failures. The skipped checks are installed-CLI handshake and the two controlled-render methods; their separate results are below. Native regressions exercise real AppKit input/UndoManager/windows/hosting views, not just string helpers. |
| Assistant `assistant-live-approval-green.log` |17 executed,16 pass,1 installed-CLI opt-in skipped. Real controlled child processes exercise initialize-only Connect, streaming/split Unicode/tool events, read-only isolation, exact known-tool approval scope, cancellation before turn ID, late events/project switch, malformed/private session/config, EOF/invalid/oversized frames, timeout, failure, bounds, new conversation and owned-child deallocation. These responses are fixtures, not model answers. |
| Installed CLI `assistant-cli-handshake.log` |1/1 pass; initialize/initialized only, no thread/model/config write. Observed `Codex Desktop/0.166.0-alpha.5.1`, arm64. This test alone does not establish working MCP/model turns. |
| `native-render.log` |2/2 opt-in methods pass. Actual native workspace/shared palettes plus assistant idle panel and command popup. Scope described below. |
| `ipc-tests.log` |7/7 runtime/socket contracts, including completion against dirty authoritative text, stale revision rejection and no history mutation, plus direct edit/save/undo and composition boundaries. |
| `mcp-tests.log`, `compiler-tests.log` |Real stdio MCP1/1 and actual TeX Live compiler integration3/3 pass. These use separate test runtimes, not the live GUI's keyboard. |
| `macos-build.log`, `macos-app-manifest.json`, `codesign.log` |Staged debug arm64 app built; strict local ad-hoc signature valid. Four packaged artifacts hashed, palette matches canonical resource, Rust archive/relink stamp matches, no newer native/shared source input. No notarization or new RAM qualification. |
| `windows-rust-check.log` |All five current Rust crates type-check for MSVC x64 after the completion fixes. No Windows native app build/runtime is implied. |
| `skill-validation.log`, `markdown-links.json` |Formal project skill validator: `Skill is valid!`.28 Markdown documents/75 relative targets,0 broken; local target existence only, web URLs/anchors excluded. |

Reproduce from the repository root using the [handoff commands](DEVELOPER_HANDOFF.md#6-reprodukálható-helyi-parancsok). The environment's sandbox rejects AF_UNIX bind and the Xcode manifest service; successful native/socket runs used scoped tool execution permission. These failures were not fixed by weakening application session authentication.

Additional opt-in commands:

```sh
export CLANG_MODULE_CACHE_PATH="$PWD/.local/clang-module-cache"
TASK_SWIFT_FLAGS=(--package-path apps/macos --scratch-path "$PWD/.local/swift-build"
  --cache-path "$PWD/.local/swift-cache" --config-path "$PWD/.local/swift-config"
  --security-path "$PWD/.local/swift-security" --disable-sandbox)
# Substitute the locally selected executable; this check does not call a model.
LATEXIDE_CODEX_HANDSHAKE="/absolute/path/to/codex" swift test "${TASK_SWIFT_FLAGS[@]}" --filter AssistantSessionTests.testInstalledCodexInitializeOnly
LATEXIDE_RENDER_PREVIEWS=1 swift test "${TASK_SWIFT_FLAGS[@]}" --filter 'IntelligenceRenderTests|WorkspaceRenderTests'
```

## Actual keyboard investigation and saved agent edit

The installed layout was `com.apple.keylayout.Hungarian`. A small native event probe measured CUA `super+z` as logical **y**, keyCode6; CUA `super+y` as logical **z**, keyCode16. Thus the earlier apparent failed shortcut was not evidence that Cmd+Z was missing. The application keeps logical-character routing; no layout setting or Cmd+Y alias was added. Probe source/raw events are under `evidence/keyboard-2026-10-31/`.

On our separate `evidence/ui-redesign/demo-project`, a real packaged MCP bridge inserted/saved a Unicode comment: baseline r3 → agent edit/save r4. Actual logical Cmd+Z restored baseline r5/dirty; actual Shift+Cmd+Z restored the saved comment r6/clean; Cmd+Z and explicit save restored baseline r7/clean. IPC assertions checked exact text/revisions/disk state (`actual-undo.json`, `actual-redo.json`, `actual-saved.json`). Redo was correctly clean because it matched the agent's previously saved content.

The subsequent focused-field regression was real: source `performKeyEquivalent` could consume another editor's undo. `KeyboardRoutingTests.testSourceDoesNotConsumeUndoIntendedForAssistantComposer` failed before the first-responder guard; native menu routing now also respects field-local UndoManager. Modified-key and saved-core-vs-composer tests pass.

## Actual final-app workflows

Only our own `evidence/intelligence-2026-10-31/demo-project` was used. The user's edited `evidence/mac-gui-project/main.tex` was preserved. The existing `--project` launch argument selected the explicit fixture after a CUA NSOpenPanel Open button remained disabled; no forced quit or user file overwrite was used. A prior renamed bundle remained running with the same identifier; after its normal quit, CUA attached to the intended new process. This is a test-tool/process-selection observation, not a claim that a folder-picker defect was reproduced.

The actual live light workspace screenshot showed `\s` candidates, coherent native chrome and editor. Down+Tab inserted `\subsection{}` with the caret in the argument; logical Cmd+Z restored `\s`, Shift+Cmd+Z restored the command, then two undo steps restored the original single backslash. The separate source fixture remained clean. The screenshot succeeded after earlier capture failures; controlled renders below remain separate artifacts.

In the assistant composer, actual Cmd+Z removed the draft and Shift+Cmd+Z restored it while the source stayed unchanged. Switching PDF→Assistant retained the draft. Connect displayed `Készen áll` for the actual installed CLI without starting a model request.

The first actual model turn streamed but failed its MCP read because `auto` approval was not unconditional. After the targeted RED→GREEN approval-scope fix and rebuild, `workspace_info` and `read_document` both displayed `completed`; the actual configured `gpt-6.1-sol` answered **A dokumentum címe: „Matematikai modellek”.** No source change/save was requested in that turn. This establishes real thread/model/MCP bootstrap for this installed CLI/account, beyond handshake or fixtures.

The next actual Codex turn read the file and inserted exactly one requested Unicode comment through `apply_edits`, without saving: baseline r1/clean → r2/agent/dirty. Source Cmd+Z restored baseline r3/clean; Shift+Cmd+Z restored comment r4/dirty. Typing then undoing text in the composer left the source at r4. Source Cmd+Z restored baseline r5/clean. Disk bytes stayed unchanged throughout. Toolbar Build then succeeded/nonstale, produced a16664-byte PDF and the native PDFKit showed the title, Hungarian text and formula; the readable live screenshot was inspected. The user's original sample hash remained unchanged. Exact text/revision/origin/dirty/disk assertions are in `live-*.json`, the operation in `live-build.json`, and the CUA observations in `live-ui-observations.json`. Reports contain no session token or private authentication data; the latter is a written observation record, not raw model-wire capture.

## Visual and product scope

`assistant-panel-dark.png` and `command-completion-dark.png` are actual native off-screen component renders (idle assistant, no fabricated transcript/model). The four refreshed workspace renders under `evidence/ui-redesign/` use actual WorkspaceView/AppKit and shared palette data; separately compiled PDFPage is vector-drawn at PDFView coordinates because off-screen cache excludes GPU tiles. `component-render-scope.txt` and `evidence/ui-redesign/render-scope.json` state that boundary. They are not live screenshots.

The command catalog is authored initial coverage, not semantic project intelligence. User macros, labels/citations, outline/definition/refactoring, SyncTeX, project content search, Git/history, file CRUD/watcher/recovery and plugin API remain planned. Ten retained documents is a prototype budget; hiding a tab preserves it rather than evicting history.

Windows11 x64 has shared Rust/source-level groundwork; the new popup and Codex adapter are Mac implementations. Windows native restore/link/run and Linux native implementation remain unrun. Darwin/AppKit process/input procedures must be adapted to each target. Broad keyboard/IME/accessibility, release RAM/long-session performance, CLI/provider matrix and student template corpus qualification remain open. Official compiler provenance does not constitute an absolute malware-free certificate.

Primary protocol/config references, consulted with the installed CLI's generated schema: [Codex app-server](https://developers.openai.com/codex/app-server), [configuration reference](https://developers.openai.com/codex/config-reference), [MCP approval resolver source](https://github.com/openai/codex/blob/main/codex-rs/codex-mcp/src/mcp/mod.rs). The exact installed alpha version must be requalified when changing it.
