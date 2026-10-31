# Native LaTeX IDE Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first local, native IDE foundation: a shared Rust document runtime, Apple Silicon macOS application, Windows 11 x64 application source, verified compiler acquisition, and direct undoable MCP editing.

**Architecture:** Each native frontend statically links a C ABI wrapper around one Rust workspace runtime. A Rust stdio MCP bridge attaches to that same runtime over a local socket/pipe; it never loads a competing copy of the document. Compilation is a separately supervised local process using official signature/hash-verified TeX Live 2026 with latexmk/XeLaTeX. Tectonic 0.17.0 remains a research baseline after exact-version advisory review.

**Tech Stack:** Rust stable 1.99.1; serde/serde_json; official rmcp 3.8.0; interprocess 2.4.7; Swift/AppKit/SwiftUI/PDFKit; C++/WinRT 3.0.260818.1 and Windows App SDK 2.6.4; TeX Live 2026 universal-darwin / Windows x64; Tectonic 0.17.0 research baseline.

**Spec:** `docs/superpowers/specs/2026-10-30-native-latex-ide-design.md` (accepted with the user's Apple Silicon-only, Windows x64, compiler-provenance, and direct-edit corrections).

## Global Constraints

- Only Apple Silicon arm64 macOS 14+; Windows 11 x64 for Intel/AMD. No Intel Mac, Windows ARM64 or 32-bit x86 target.
- No embedded browser or Node/.NET requirement for the native product.
- Rust stable only; downloaded tools, dependencies and runtime caches stay in the project during development. Do not alter the user's shell configuration or system PATH.
- Per text file: at most 1 MiB; at most 10 open files. Preserve UTF-8 BOM and LF/CRLF on save; reject invalid UTF-8 without writing.
- Public edit positions: zero-based line and UTF-16 column, revision protected. Direct MCP edits share GUI undo/redo and survive saving in the undo history.
- Single IPC message: at most 1 MiB encoded. Document reads are chunked at 64 KiB raw UTF-8.
- One build per workspace. Default timeout 120 seconds; configurable 30–600 seconds. Keep the last successful PDF visibly distinguishable after a failed build.
- Official release URLs, fixed versions, upstream hashes and local verification evidence for acquired compilers. Hash equality is not a malware-free certification.
- No publication, cloud resources, Git initialization or commits are part of this local implementation. No Windows runtime success may be inferred from macOS checks.

## Review Focus

- UTF-16 edits around emoji/combining characters and CRLF must not split or silently alter text; Task 2 owns these tests.
- An agent edit followed by user input, save and undo must retain ordering and not erase unrelated user changes; Tasks 2/5 own these tests.
- External file changes and failed writes must not be reported as successful saves; Task 2 owns these tests.
- An MCP client with a wrong session token or project traversal must not access another workspace; Task 3 owns these tests.
- A failing/cancelled compiler must not leave orphan processes or relabel an old PDF as fresh; Task 4 owns these tests.

## Task 1: Acquire development tools and compiler with provenance

**Files:** `scripts/acquire_tools.py`, `tools/toolchain-lock.json`, `.gitignore`, `evidence/acquisition/`, `docs/compiler-provenance.md`.

**Interfaces:** Produces `.local/rust/bin/{cargo,rustc}`, `.local/compilers/macos-arm64/tectonic`, `.local/compilers/windows-x64/tectonic.exe`, a JSON manifest of exact URLs/upstream digests/observed digests, and local cache directories. The acquisition script accepts `--metadata-only` and `--install`.

- [x] Write the acquisition manifest from the already retrieved upstream release metadata. Select only `aarch64-apple-darwin` and `x86_64-pc-windows-msvc` Tectonic assets. Pin stable Rust components, including Windows x64 std for cross-checking.
- [x] Implement streaming download to a temporary file, SHA-256 verification before extraction/execution, bounded safe extraction that rejects absolute/traversal links, and atomic promotion to the local tools directory.
- [x] Run the acquisition using the official sources. Expected: recorded upstream/local hashes match; native Mac compiler architecture and Windows PE architecture match their target. Unknown/unavailable malware checks remain explicitly unknown.
- [x] Run Rust/Tectonic version checks and save evidence. Expected: 1.99.1 / 0.17.0. Do not count a version command as a successful TeX compile.

## Task 2: Shared Rust documents and safe persistence

**Files:** root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`; `crates/latexide-core/{Cargo.toml,src/lib.rs,src/document.rs,src/workspace.rs,tests/document_contract.rs}`; `tests/fixtures/`.

**Interfaces:** `Position { line: u32, character: u32 }`, `TextEdit { start: Position, end: Position, text: String }`, `Workspace::open(root: PathBuf)`, `open_document(relative_path)`, `snapshot(document_id)`, `apply_edits(document_id, expected_revision, edits, origin)`, `undo()`, `redo()`, `save_document(document_id, expected_revision)`. Return structured errors, transaction IDs and changed document snapshots. Concrete Rust result types live with these interfaces.

- [x] Write tests first for Unicode edits, invalid surrogate boundaries, stale revisions, user/agent undo order, save-then-undo, BOM/CRLF byte preservation, invalid UTF-8, root escape and a changed-on-disk save conflict.
- [x] Run the named integration test before implementation. Expected: failure from missing interfaces, not a test that already passes.
- [x] Implement bounded document state, incremental transactions and history; keep undo after save. Validate all changes before mutating a transaction. Track original saved bytes to detect file conflicts without relying solely on watcher timing.
- [x] Implement single-file atomic replacement and dirty state. Expected: a write failure leaves dirty content and the prior successful saved baseline intact. A clean file may save as a no-op after the external-change check.
- [x] Run `cargo test -p latexide-core`. Expected: all contract tests pass. Record any platform-specific save limitation rather than claiming a universal filesystem guarantee.

## Task 3: Versioned runtime protocol, C ABI and same-session IPC

**Files:** `crates/latexide-protocol/`, `crates/latexide-ffi/`, `include/latexide_core.h`, `tests/ffi_contract.c`, `tests/ipc_contract.py`.

**Interfaces:** ABI version 1; opaque `LatexideRuntime*`; `latexide_runtime_new(const char *project_root)`, `latexide_runtime_free(handle)`, `latexide_dispatch(handle, const char *request_json) -> char*`, `latexide_string_free(char*)`. JSON replies are `{ok:true,result:...}` or `{ok:false,error:{code,message}}`; request operations share Task 2 types. Runtime requests: `workspace_info`, `list_project_files`, `open_document`, `read_document`, `apply_edits`, `save_document`, `undo`, `redo`, `get_diagnostics`, `request_build`, `get_operation`. IPC wraps the same request with a session nonce and protocol version.

- [x] Write C ABI tests for lifecycle, malformed JSON, allocated response freeing and an agent-edit/undo round trip. Write IPC tests for wrong token, traversal and same-runtime state sharing.
- [x] Run before implementation. Expected: absent header/library/endpoint causes an explicit failure.
- [x] Implement a single shared runtime with narrow panic-contained C entry points, bounded messages and versioned errors. Only short document operations may be synchronous; builds execute off the UI thread.
- [x] Implement user-scoped local socket/pipe discovery and shutdown. Use Unix domain socket on macOS and named pipe on Windows; keep secrets out of arguments/logs. A stopped runtime removes its own session descriptor, not unrelated sessions.
- [x] Run the C harness, core tests and IPC tests. Expected: all pass locally; unsupported Windows-only runtime checks remain unrun.

## Task 4: Compiler supervisor and actual TeX/PDF evidence

**Files:** `crates/latexide-build/`, `scripts/compiler_smoke.py`, `tests/fixtures/basic/`, `tests/fixtures/multifile/`, `evidence/compilation/`.

**Interfaces:** `BuildRequest { root, main_file, compiler_path, output_directory, timeout_seconds }`; `BuildStatus` with `id/state/exit_code/pdf_path/log/diagnostics/source_revisions/stale`; `BuildSupervisor::start`, `status`, `cancel`. Attach one supervisor to Task 3 runtime. PDF output path is immutable per successful build ID.

- [x] Write tests using a controlled child process for nonzero exit, timeout, cancellation and prevention of concurrent workspace builds; use real TeX fixtures for successful/failing output semantics.
- [x] Run the controlled-process tests first. Expected: missing supervisor fails.
- [x] Implement command-array invocation with no shell, Tectonic `--untrusted`, explicit cache/output directories, bounded logs, process-tree termination and accurate state transitions. Add optional TeX Live argument preparation; never silently fall back to an unverified arbitrary executable.
- [x] Compile the basic and multifile fixtures using the acquired Mac compiler. Repeat with cached-only/offline operation. Expected: nonempty valid PDF; intentional error fails; old PDF remains tagged to the earlier build.
- [x] Record the Windows compiler as acquired/hash-verified only until actually run on Windows 11 x64.

## Task 5: Apple Silicon native application and direct MCP workflow

**Files:** `apps/macos/Sources/`, `apps/macos/Info.plist`, `scripts/build_macos.sh`; `crates/latexide-mcp/`; `scripts/mcp_smoke.py`.

**Interfaces:** Native UI uses Task 3 ABI. Swift owns presentation/focus state; Rust owns content/revisions/history. `latexide-mcp --session <descriptor-path>` reads the protected descriptor and uses official rmcp stdio transport; its tools map directly to the shared runtime. No session nonce in command arguments.

- [x] Write MCP process tests for initialize/discovery, tool listing, reading dirty text, direct agent editing, saving and GUI-equivalent undo. Expected: no review/proposal acknowledgement is needed to apply a valid edit.
- [x] Implement the bridge using rmcp 3.8.0 and test it against its official client or the smoke harness. Keep all ordinary logs on stderr.
- [x] Build the SwiftUI application with an AppKit text adapter and PDFKit. Implement project/file opening, revision-aware incremental edits, native shortcut routing to Rust undo/redo, explicit saving, build status and PDF generation updates. Disable smart quote/dash source transformations. Native IME composition must commit once.
- [x] Produce a local arm64 `.app` that includes or locates the acquired compiler and has no developer-home hardcoded product paths.
- [ ] Run and inspect the rendered application, then perform the actual agent-edit → visible update → undo → save → compile → PDF cycle. Record what was actually exercised and process memory; build success alone is insufficient.

## Task 6: Windows 11 x64 native application and validation boundary

**Files:** `apps/windows/LaTeXIDE.sln`, `apps/windows/LaTeXIDE.vcxproj`, `apps/windows/packages.config` or pinned `PackageReference` equivalents, `apps/windows/{App,MainWindow}.{xaml,xaml.h,xaml.cpp,idl}`, `apps/windows/NativeCore.{h,cpp}`, `scripts/build_windows.ps1`, `docs/windows-validation.md`.

**Interfaces:** x64 only; same Task 3 header and protocol. Pin Windows App SDK 2.6.4 and C++/WinRT 3.0.260818.1. Link the Rust MSVC static library; keep editor behavior behind an adapter. Native PDF baseline uses Windows.Data.Pdf, without promising text selection/search.

- [x] Create a Rust Windows target check with `cargo check --workspace --target x86_64-pc-windows-msvc`. Expected: all Rust Windows code type-checks; this is not a linked Windows application.
- [x] Implement the WinUI application source, native core bridge, plain-text RichEditBox adapter, undo/redo routing, file commands, compiler status and PDF page view. Handle UTF-16 and native paragraph conventions explicitly.
- [x] Provide a deterministic Windows build script that resolves only the pinned dependencies and copies the acquired x64 compiler.
- [ ] On a real Windows 11 x64 environment, build, launch and exercise the same direct-agent-edit/save/undo/compile/PDF cases. If no environment is available, mark this step unrun and the full two-platform acceptance incomplete; continue all independent local work.

## Task 7: Evidence and final independent review

**Files:** `docs/implementation-progress.md`, `docs/verification.md`, `README.md`, `evidence/`.

- [x] Run the current Rust test suite, C/IPC/MCP integration checks, the Mac build and real compiler checks. Record exact commands/results and the remaining Windows verification gap.
- [x] Request one fresh-context whole-change review against the accepted spec and this plan. Fix material findings with a reproducing test and rerun the affected suite; routine documentation fixes need no new tests.
- [x] Update the README with actual run/build commands, implemented/partial/planned status, compiler provenance and undo semantics. Do not claim malware certification or Windows runtime success without evidence.
- [x] Report the actual artifact, tests and remaining external prerequisites. Preserve local evidence and the progress ledger; there is no Git history to recover it from.

## Execution notes

The user's conditional acceptance authorizes the described local implementation. The implementation method is direct work in this session with one final independent review; no additional architecture approval is requested. The workspace is not a Git repository, so Git-specific helper bookkeeping/commits are replaced with `docs/implementation-progress.md` and local evidence. Publication, system-wide installation and Windows/cloud resources are outside this plan.

## Maintained developer handoff (user addition)

After every substantive phase, update `docs/DEVELOPER_HANDOFF.md` and `docs/implementation-progress.md`; update `skills/latexide-platform-development/references/` when reusable platform contracts or procedures change. Record actual Mac checks and Windows/Linux implications. Windows runtime is unavailable; Linux UI is a future unselected port.

## Final acceptance boundaries

The pre-review Mac GUI/MCP workflow and debug RSS were observed; post-review native tests/build/signature verification passed. Fresh post-review visual inspection was blocked by CUA ScreenCaptureKit -3812. Windows restore/link/runtime remains unrun because no machine exists. These open acceptance checks do not block delivery of the requested maintained Markdown/skill or local source artifacts, but the full two-platform/student-release objective is not complete.
