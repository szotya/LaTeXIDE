# Developer UI verification — updated 2026-10-31

The native developer workspace, preset palettes and lexical LaTeX coloring are implemented. Tests, controlled renders, live observations and unrun target checks are separate evidence. This supplements the historical [foundation verification](verification.md), not a full student-release acceptance.

**Newer increment:** [keyboard, command completion and actual Codex assistant verification](intelligence-verification.md). It closes the earlier logical Cmd+Z and post-build PDFKit/readable live capture gaps. Counts, final bundle manifest and present capability status in that document supersede the historical phase results below; the old observations remain unchanged as evidence.

## Current evidence

Paths below are under `evidence/ui-redesign/` unless stated otherwise.

| Evidence | Actual result and scope |
| --- | --- |
| `rust-tests.log` | 29 pass:13 document,8 syntax,8 build-supervisor. Includes escaped/comment/math/verbatim context, prose titles/text in math, scalar/window boundaries and a late window in840000 bytes. The earlier roughly32ms debug lexer observation is not app latency/RAM qualification. |
| `native-tests.log` | 15 pass;1 opt-in render skipped. Existing UTF-16/history/IME/project contracts plus palette contrast/persistence, attribute-only recolor, marked-text deferral, tabs and actual viewport/selection restoration. |
| `viewport-behavior-red.log` | Actual native regression before restore fix: selection `{4,10}` collapsed and viewport y850 became y0, also after final-tab recreation. Both new tests now pass. `viewport-red.log` is only the earlier absent-API compile failure. |
| `native-render.log` | Opt-in controlled render1/1 passes after review fixes; four2880×1800 PNGs. Actual WorkspaceView/AppKit editor/shared palettes, isolated preferences, separate demo project. Real compiled PDFPage vector is drawn at PDFView coordinates because off-screen cacheDisplay excludes GPU tiles. Native traffic lights/live interaction excluded. Exact scope:`render-scope.json`. |
| `ipc-tests.log` | 6/6 actual runtime/socket tests, including revision-protected dirty-source syntax, nonmutation, stale revision rejection and existing direct-agent/composition/save/undo contracts. |
| `mcp-tests.log` | 1/1 real stdio roundtrip: dirty read, direct edit, explicit save, native-side shared undo, stale revision. Separate test runtime, not a second live GUI keyboard cycle. |
| `compiler-tests.log` | 3/3 real TeX Live2026/latexmk4.88/XeLaTeX integration tests: PDF/revision, native compiler configuration boundary, credential-env filtering. |
| `windows-rust-check.log` | All five Rust crates MSVC x64 type-check. No native link/runtime. |
| `windows-ui-source.log` | 5 XML/42handlers/reference checks pass. Not XAML generation or native C++/WinRT compilation. |
| `windows-editor-helpers.log`, `windows-text-adapter.log` | Both pure C++20 harnesses compile/run on Mac with warnings treated as errors: scalar windows/lines/paragraphs/UTF-16 and per-project initial main-file choice. Not RichEdit behavior. |
| `macos-build.log`, `macos-app-manifest.json`, `macos-linked-libraries.log` | Final debug arm64 app built/staged with compiler, bridge and SwiftPM palettes. Strict local ad-hoc signature verified; canonical palette bytes and Rust archive relink stamp match. No notarization/new RAM result. |
| `resume-2026-10-31/` | Fresh bundle/hash/source-input comparison, strict signature and skill validator pass;17Markdown documents/51relative targets,0broken. Saved old runtime inspected and closed normally. New final app: real MCP edit/save r2, visible native-menu undo r3/dirty, explicit native-menu save, toolbar-triggered successful nonstale PDF. Final post-build AX observation was blocked by capture failure. See exact sequence below. |

## Actual live UI observations

The original user's `evidence/mac-gui-project/main.tex` was saved at r25, including their added prose. Its source was preserved. The old GUI quit normally with Cmd+Q. The new UI launched from `dist/LaTeXIDE.app`; we opened the separate `evidence/ui-redesign/demo-project` through NSOpenPanel.

Live CUA accessibility-tree observations:

- Compact native chrome/activity/project/tree/tabs/breadcrumb/gutter/PDF/output/status and actual source.
- Build used **packaged** TeX Live and produced one page. PDFKit exposed “Matematikai modellek”, Hungarian prose and equation content; UI showed success and1/1 page. Latexmk's upstream uninitialized-value warning preceded the successful run and remains in the log.
- Appearance popover had System/Light/Dark and all four palettes. Ocean and fixed Light were clicked; later live state still showed Ocean. Native tests separately cover persistence/mode semantics/all8 variants' contrast. Live OS theme change was not exercised.
- Cmd+P displayed Quick Open; filtering found `chapters/modell.tex`, Enter opened a second tab. Cmd+F displayed native NSTextFinder. The search field was explicitly focused for typing, so this is not proof of every keyboard/focus path.

On2026-10-30 CUA intermittently failed with ScreenCaptureKit -3811/-3812 and returned white/tiny distorted images. It also reported intervening user interaction; a later live tree showed further font-size/PDF-zoom choices, which were preserved. A readable live screenshot was not available. The final viewport fix was tested/rendered after that live launch; the active instance was left intact at the pause. It was subsequently closed normally during the resumed checks below.

The foundation's earlier actual MCP edit/save → GUI-menu undo/save → bundled compile/PDF cycle remains documented in [verification.md](verification.md). The redesigned GUI now has its own live shared-history sequence below, with its remaining final PDF-display limitation stated separately.

## Resumed final-app workflow — 2026-10-31

No application source changed during documentation closure. Four packaged artifacts matched the existing UI manifest, the current Rust archive matched its Swift relink stamp, and no source input was newer than the verified build. The2026-10-30 suites were not rerun merely because the session resumed.

1. Read-only IPC confirmed the existing GUI's two documents were clean and its build succeeded (`before-restart.json`). Quit normally with Cmd+Q and launched the final canonical bundle through CUA; reopened the separate demo through NSOpenPanel. This normal restart reset ephemeral tab/zoom state; stored appearance/font preferences were not changed by the resumed checks.
2. A real stdio MCP process using the packaged bridge inserted `% MCP GUI edit: Árvíz 😀` at the start of `main.tex` and explicitly saved r2 (`live-mcp-edit.log`). CUA showed that exact comment in the native editor.
3. A CUA `super+z` attempt did not produce an observed edit. Native **Edit → Visszavonás** visibly removed the comment; read-only IPC proved r3, baseline source restored, dirty=true, while disk still contained the agent's saved comment (`live-undo.json`). The native menu is the verified action; hardware shortcut behavior remains unqualified.
4. Native **File → Összes mentése** restored the disk baseline and clean state. Toolbar **Fordítás** triggered packaged TeX Live. Its capture call failed with ScreenCaptureKit -3812, but IPC proved `succeeded`, `stale=false`, a16665-byte valid PDF and saved source r3 (`live-build.json`, `live-final-state.json`). The original user sample's SHA-256 was unchanged throughout.
5. A rebind to observe final PDFKit display failed again with -3812. Thus the final app's MCP/native-menu undo/save/compile pipeline is verified; its post-build PDFKit display and readable live screenshot are not newly observed. The prior live PDFKit observation and controlled native renders remain separate evidence. The final app is left running with a clean, restored demo source.

All files named in this section live under `evidence/ui-redesign/resume-2026-10-31/`. Session records contain the descriptor path only, not its token. `baseline-source.txt` is our demo, not the user's original project.

## Reproduction

From the repository root, using the project-local toolchain/compiler:

```sh
bash scripts/cargo-local.sh test --workspace --locked --offline
bash scripts/cargo-local.sh build -p latexide-ffi -p latexide-mcp --locked --offline
CLANG_MODULE_CACHE_PATH="$PWD/.local/clang-module-cache" swift test --package-path apps/macos --scratch-path "$PWD/.local/swift-build" --cache-path "$PWD/.local/swift-cache" --config-path "$PWD/.local/swift-config" --security-path "$PWD/.local/swift-security" --disable-sandbox
python3 tests/ipc_contract.py
python3 tests/mcp_contract.py
python3 tests/compiler_runtime_contract.py
bash scripts/cargo-local.sh check --workspace --target x86_64-pc-windows-msvc --locked --offline
python3 apps/windows/tests/check_ui_source.py
```

Native builds/tests and socket tests needed scoped execution permission in the Codex sandbox. Do not weaken authentication/revision checks to avoid EPERM. Windows helper commands/prerequisites are in [windows-ui-port.md](windows-ui-port.md).

Controlled renders require maintained demo files and the real separately compiled `demo-build.json`/PDF, not the user's sample:

```sh
LATEXIDE_RENDER_PREVIEWS=1 CLANG_MODULE_CACHE_PATH="$PWD/.local/clang-module-cache" swift test --package-path apps/macos --scratch-path "$PWD/.local/swift-build" --cache-path "$PWD/.local/swift-cache" --config-path "$PWD/.local/swift-config" --security-path "$PWD/.local/swift-security" --disable-sandbox --filter WorkspaceRenderTests
bash scripts/build_macos.sh
codesign --verify --strict dist/LaTeXIDE.app
```

Staging retains the prior bundle under `dist/LaTeXIDE.previous.*.app`. Canonical `script/build_and_run.sh` refuses an existing process. Save/quit normally before launching the final bundle; do not terminate a user's unsaved runtime.

## Remaining qualification

Windows restore/XAML/C++/WinRT link, RichEdit/IME/clipboard/system-theme/DPI/accessibility/compile/PDF/IPC/Job Object runtime remain unrun. Filename-filter/action-menu/PDF-page rendering is partial parity, not a fuzzy popup/searchable command palette/PDFKit text search. Linux UI/toolchain is unimplemented.

Mac hardware input/IME/undo shortcut, live OS-theme switching, complete accessibility, resize extremes, long-source app/RAM benchmarks, release packaging and the final post-build PDFKit observation remain unqualified. Lexical coloring is not semantic parsing. Hidden tabs still occupy the ten-document core budget. Project content search, disk watcher/recovery/full build manifests and existing compiler release/corpus/security qualification remain later work.

Once-only review and rulings: [developer-ui-review.md](developer-ui-review.md). Shared continuation contract: [UI skill reference](../skills/latexide-platform-development/references/ui.md). The central handoff and ledger carry the maintained actual state.
