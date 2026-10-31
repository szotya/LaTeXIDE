# Local foundation verification — 2026-10-30

This is an M0/M1 foundation with a tested Mac/MCP path. Full student-release acceptance and Windows runtime verification are incomplete.

This table records the foundation phase. The subsequently implemented developer UI, multiple palettes, lexical coloring and current evidence are in [ui-verification.md](ui-verification.md).

| Evidence | Result / actual scope |
| --- | --- |
| `evidence/rust-workspace-tests.log` | 21/21 passed after review fixes: 13 core and 8 supervisor contracts. |
| `evidence/composition-green.log` | 5/5 actual IPC tests passed, including composition exclusion, commit-once and rejected-commit recovery. |
| `evidence/mcp-green.log` | Real rmcp stdio initialize/list/read-dirty/direct-edit/save/native-undo/stale-revision round trip passed. |
| `evidence/compiler-runtime-tests.log` | Three tests passed: real TeX Live PDF + stale open-document revision, agent cannot replace compiler, credential-like env sentinel does not reach child. |
| `evidence/swift-tests.log` | 7/7 passed: UTF-16, scalar delta, project open/switch, commit-once, native key-routing and rejected-IME recovery. |
| `evidence/macos-app-build.log` | Native arm64 SwiftPM GUI linked to the Rust static archive; local `.app` staged with TeX Live and MCP bridge. Ad-hoc development signature, not notarized distribution. |
| `evidence/mcp-gui-probe.log` | A separate actual stdio MCP process edited and saved the running GUI runtime, revision 2. |
| Native UI inspection | Folder opened using NSOpenPanel. Agent comment visibly appeared in NSTextView. Native Edit → Visszavonás removed it after save (r3, dirty). Toolbar save/build produced succeeded and a rendered PDFKit page whose Hungarian text/math was visible in the accessibility tree. |
| `evidence/macos-memory-debug.txt` | One small project + one-page PDF: 174080 KiB RSS = 170 MiB for the IDE process, debug build, Mac15,7/macOS27.2. Single snapshot, not release/performance qualification or compiler/agent memory. |
| `evidence/windows-rust-check.log` | Latest run checks all five Rust crates for x86_64-pc-windows-msvc; no native Windows link or runtime. |
| `evidence/windows-text-adapter.log` | Pure UTF-16/paragraph functions compiled/executed with Mac Clang. Not a Windows control result. |
| `docs/skill-validation.md` | Formal project skill validation + independent baseline/forward onboarding checks. |

## Observed failures fixed

- Swift's NSRange conversion alone accepted a split surrogate; explicit scalar-boundary validation now rejects it (failing test retained in `swift-adapter-red.log`).
- Real project open initially cast FileEntry objects as strings and aborted. A native-store integration test reproduced it (`swift-project-red.log`); the adapter now decodes each entry's path with validation.
- The native XCTest host then exposed SIGPIPE on session shutdown, hidden by Python's default ignored SIGPIPE. Shutdown now wakes accept by connection only; macOS IPC writes set SO_NOSIGPIPE per socket rather than changing the host's signal policy.
- The default execution sandbox denied the Xcode build service's manifest write and local IPC bind. Scoped build/test permissions were used; authentication and platform checks were retained.

## Remaining qualification

Windows linked build, GUI/runtime/compiler/IPC/Job Object tests; real keyboard/input-method testing beyond the adapter test; release RAM/corpus/stress/accessibility; full disk-change watcher/recovery, project content search and packaging/notarization. Lexical syntax coloring and native in-file Find are now implemented. Polling compact workspace metadata every250ms is the current Mac prototype update mechanism; callbacks/event subscriptions remain future work. External raw filesystem edits do not enter the IDE's undo history. Current build staleness tracks open in-memory revisions; a complete external file manifest is not yet implemented.

The authoritative current source map, commands and platform caveats are in `docs/DEVELOPER_HANDOFF.md`. No publication or system-wide tool installation was performed.

Independent review and author dispositions: `docs/foundation-review.md`. TeX Live relocation profile generation: `evidence/review-installer-green.log` (1/1). Full reinstall at a second root remains unrun.

Foundation package snapshot: `evidence/macos-app-manifest.json`, arm64. The current redesigned UI package manifest is `evidence/ui-redesign/macos-app-manifest.json`; see [current UI evidence](ui-verification.md). Full `codesign --verify --strict` passed, exit 0 (`macos-codesign-check.log`). An optional dangling TeX Live `man` alias initially caused the resource verifier to report ENOENT; the build omits only that unused alias from the generated bundle and verifies all resources. The source compiler installation is unchanged.

The foundation's final post-review GUI reinspection was blocked by ScreenCaptureKit `-3812`, including after reset. Subsequent UI development and the 2026-10-31 resumed live MCP/menu-undo/save/build observations are recorded in [ui-verification.md](ui-verification.md); its exact remaining capture boundary still applies.
