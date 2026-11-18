---
name: latexide-platform-development
description: Use when developing this LaTeX IDE's shared Rust core, native macOS or Windows frontend, local MCP bridge, compiler integration, or a future Linux port, including onboarding from the Mac implementation.
---

# LaTeX IDE platform development

Use the accepted project contracts and current evidence when continuing work across platforms. Locate the repository root first; paths below are repository-relative, even if this skill has been copied into a personal skill directory. Resolve those paths against the located checkout. Repository-document hyperlinks use published GitHub URLs so they also work in copied skills; for source/evidence work prefer the matching document in the checkout. Links within this skill remain relative to the copied skill folder.

## Start from maintained context

Read `docs/DEVELOPER_HANDOFF.md` for current implementation state, actual commands, compiler provenance, known Mac-specific assumptions and remaining platform checks. Read the active task in `docs/implementation-progress.md` and its referenced plan before editing. The latest user corrections in the accepted design take priority over older proposals.

| Work | Additional reference |
| --- | --- |
| Windows 11 x64 / C++/WinRT / WinUI 3 | [references/windows.md](references/windows.md) |
| Future Linux port / Unix IPC / native UI choice | [references/linux.md](references/linux.md) |
| Native workspace, shared themes, lexical syntax and editor presentation | [references/ui.md](references/ui.md) |
| Logical keyboard undo, command completion and local Codex assistant | [references/intelligence.md](references/intelligence.md) |
| Project macros, ref labels, BibTeX keys, cache and fresh acceptance | [references/project-completion.md](references/project-completion.md) |
| Own macro/ref/cite definition lookup and native Cmd/Ctrl-click | [references/definition-navigation.md](references/definition-navigation.md) |
| Capability audits, source/test evidence and IDE parity backlogs | [references/audit.md](references/audit.md) |
| Durable recovery, explicit Save/undo and qualified parent-owned crash cohorts | [references/recovery.md](references/recovery.md) |
| Parallel development packages, exclusive paths, contract gate and GitHub onboarding | [references/parallel-development.md](references/parallel-development.md) |
| Compiler acquisition, package roots and offline proof | `<repo>/tools/texlive/README.md` and `<repo>/docs/compiler-assessment.md` |

## Windows/Linux contributor entry

Mac is the authoritative accepted product/workflow reference. Read the [platform contributor policy](https://github.com/szotya/LaTeXIDE/blob/main/docs/development/PLATFORM_DEVELOPMENT.md), [parity baseline](https://github.com/szotya/LaTeXIDE/blob/main/docs/development/PLATFORM_PARITY.md) and target app AGENTS/README before port work. Starting branches are `szotya/p21-windows` and `szotya/p22-linux`; later tasks use short branches from current main.

P21 owns the complete `apps/windows/` native tree, including existing MainWindow/NativeCore/XAML/project/NuGet files, its native tests and Windows build script. P22 owns the complete `apps/linux/` tree, its native tests, self-contained app/probe manifests/locks and future Linux build script. The generated ownership registry is authoritative for exact paths. Older blanket P00 ownership of existing WinUI files is superseded by this 2026-11-03 policy.

Shared Rust/runtime/session/IPC/C ABI/MCP, root workspace/locks, shared themes/workflows and central handoff/ledger/skill stay P00 integration work. Platform owners write their own result journal and request shared hooks through their own manifest. Bind each parity result to the exact Mac reference document/test and base commit. The platform test scripts named in the plans are future implementations until their files and real target executions exist; preparation must not add passing placeholders or imply native qualification.

## Preserve the cross-platform contract

- One authoritative Rust workspace runtime. Native controls and MCP clients share it.
- Zero-based line/UTF-16-column edits, expected revisions, and atomic in-memory validation. Preserve Unicode, BOM/CRLF and external-change checks.
- Native marked text uses `begin_composition` / `end_composition`; end consumes the transaction even when validation rejects it, so restore acknowledged text and show the error.
- Agent edits apply directly, without per-edit approval. Save is explicit and does not erase shared undo/redo. Arbitrary external filesystem writes have a separate conflict boundary.
- C ABI uses opaque handles and Rust-owned buffers with the matching free function; avoid full-document serialization for each keystroke.
- Keep build failures, cancellation, timeouts, old PDFs and source revisions distinguishable. Compiler processes receive a restricted environment, not agent credentials.
- Mac target is Apple Silicon only. Windows target is x64 Intel/AMD only. Linux remains a future port until its own UI/build/runtime checks exist.

## Keep the handoff current

The user requested ongoing Markdown and skill documentation of Mac development for Windows and Linux developers. After a material code, toolchain or verification change, update the handoff and ledger with: change and reason; relevant paths; actual command/result; platform implications; remaining verification. Update this skill's target reference when a reusable contract or procedure changes. Keep the chronological transcript in the Markdown ledger rather than duplicating it here.

Before reporting a platform result, distinguish source, type-check, linked build, tests, launch and rendered UI evidence. Record missing checks as unrun. Follow the TeX Live evidence's actual scope: signature/hash equality is provenance and integrity, not a malware-free certificate. The current Tectonic assessment is not a production qualification.

Use paths and commands for the target OS. The Mac bootstrap and `/private/tmp` convention are not Linux/Windows implementations. Inspect existing scripts before adapting them; preserve current user intent without adding publication, account creation or system-wide installation to an onboarding task.


## Read-only bibliography library

`crates/latexide-core/src/services/bibliography/` exposes the existing v1 BibliographyPort/DTO as a local library. Preserve the lossless byte partition, logical FEFF/CRLF, UTF16 ranges and exact case-sensitive duplicate reporting. Malformed headers recover at their line boundary. Delimited `@comment` payloads balance matching delimiters and treat quotes/percent signs literally, so a closed comment cannot consume the following entry through value-scanner state. Keep `@string`, `@preamble` and ordinary value scanning separate from this comment rule. Fragment offsets are borrowed UTF-8 byte ranges; report ranges are UTF-16 code units. This does not change completion/definition winner rules or resolve macros, crossrefs, xdata or schemas.

Read `docs/development/results/P12.md` and `integration/requests/P12.json` before extending it. Diagnostics currently use bibliography.bib/revisionNone plus the supplied input digest; a future host integration must bind the authoritative document and revision. The historical binding.json and frozen receipts stay historical; current canonical library/Cargo/lock pins belong to a separately verified integration cohort. The two exact-marker should_panic guards retain their negative role and must reject precondition panics. A fresh-source review and a new-main integration are separate results; neither implies UI/Runtime/network import, native Windows/Linux, SDK38 strong closure or production acceptance. Keep LANG-11 partial and issue13 open until its full assertions are qualified.

Exact-byte manifest pins require reproducible checkout bytes: the root attributes force LF for the three pinned Cargo files, while bibliography fixture attributes preserve intentional CRLF. Keep raw-byte hash assertions; do not silently normalize their inputs. Libtest reports the standard ` - should panic` display qualifier before ` ...`; retain the canonical method ID and every named-outcome/count/duplicate/private-child guard when parsing it. A failed or ignored expected-panic method cannot satisfy required evidence. The focused Python contract test covers this adapter separately from actual Rust guard execution.

## Malformed proc-macro metadata fixtures

`tools/verification/proc_macro_metadata_fixture.py` is a narrow Python test utility, not a compiler launcher or toolchain locator. The trusted caller supplies the positive metadata basename; only an own private nonce owner and frozen31-byte source may create its new bad fixture. Preserve descriptor/no-follow, mode/owner/single-link and exact-byte/hash checks. Unsupported descriptor backends refuse before path handling. The E0786 classifier checks content against the supplied locator; matching JSON does not authenticate compiler origin, and diagnostic text must never be executed or opened.

Use the pinned synthetic diagnostic template and a host-native absolute locator in portable tests; keep historical raw stderr and personal paths outside published source. Nine POSIX fixture methods are explicitly unavailable on Windows, while the diagnostic and fail-closed controls still run. The existing strict CI summary validator accepts only those exact new skip identities on win32; other skips/count mismatches still fail. Keep skipped POSIX behavior distinct from passing portable content checks and native platform acceptance. Read `docs/development/results/P00.md` for the actual cohorts and commands; frozen review, current-main integration, exact-head hosted CI and actual-main CI remain separate results.

Permission-negative fixtures must set their intended invalid modes with chmod after creation: a restrictive umask can otherwise turn0755/0644 into valid0700/0600. C10 runs this regression under077 and restores its caller's umask through test cleanup. Limit those mode changes to the test's own newly created fixtures.


## Canonical bibliography project index

The private `project_index` path-module at `services/bibliography/index_adapter.rs` projects accepted entries from the unchanged canonical library into existing Workspace completion/definition symbols. It only locates accepted header key bytes; it does not parse a second bibliography or forward the standalone synthetic diagnostic anchor. Keep fragment/report pairing when filtering invalid keys, percent-header trivia, mixed-case/parenthesized entry types, UTF8 key-byte caps and logical-text UTF16 coordinates. Preserve real document/revision binding, dirty/open/disk precedence, last-per-file winner and cross-file alternatives, cache and partial budgets. The canonical26 fixture/oracle/Cargo pins remain unchanged; register only the new bibliography_index_contract test path in P12 ownership and request project_index as an explicit P00 hook. Run the new Workspace contracts alongside canonical, project_completion and definition contracts with offline/locked/jobs1; bind exact discovery and the two expected panic markers to the actual compiled library. Read the scoped result in `docs/development/results/P12.md`. This integration does not close full P12/import/UI or native Windows/Linux acceptance.

For disk freshness, length/mtime alone cannot identify decoded source content. After the existing TTL or explicit refresh, hash the bounded logical text with the unchanged fixed-block helper and retain that same text for a parse miss. Preserve the 1 s warm window, the three source-sized decoder allocations, dirty/open revision precedence and all partial budgets; unchanged content should reuse parsed symbols even after explicit refresh. Register `bibliography_index_freshness_contract.rs` as one exact P12 source path without replacing existing owners or tests. Its persistent fixtures need process-local unique names, and its process-wide allocation observer requires internal serialization even under ordinary parallel libtest execution. Qualify shared-process execution as well as exact discovered method identities; per-method selection alone can conceal fixture collisions. Keep grouped process time/RSS measurements distinct from individual measurements, and never replay completed methods to repair a result reader. The 8 freshness contracts join the retained 73 contracts; native desktop and full parent acceptance remain separate.

Parse reuse compares the logical digest and symbol budget; metadata-only or physical BOM/CRLF rewrites of the same logical text should refresh stored metadata without reparsing. Preserve raw-size admission and check decoded aggregate bytes before both hits and misses. Hash every actual disk read, including warm budget misses, so cached symbols cannot carry an older digest. Use private clock/read-source seams for fixed pre-expiry and metadata/read-transition regressions; production must keep the real clock and bounded reader behind the unchanged Workspace API. Retained fixture-name collisions retry exclusive creation without touching old evidence. Recover a unit-valued serialization guard after poisoning and reset process-wide allocation observation with Drop. Distinguish caught fixture panic controls, exact-marker canonical guards, selected results and unselected library methods.


## Active raw-source character count

UI56 (`#56`) counts the complete active raw source in `Editor/SourceCharacterCount.swift`, using Swift `Character` extended grapheme clusters and `Character.isWhitespace`. Combining marks/ZWJ emoji count as one; CRLF is one grapheme. LaTeX commands/comments remain included; the separate body-count request `#69` must not silently change this contract. Preserve the unchanged source bytes and revision/undo/save semantics.

`WorkspaceView` must pass its existing `EditorPreferences` instance into the observed status bar. The default-on `ide.showSourceCharacterCount` preference uses a compatible forwarding storage adapter; keep every older key/default/clamp and the public UserDefaults constructor. Use injected memory storage for headless tests. Memory recreation is not live UserDefaults/app-restart acceptance, and seeded store state is not Runtime/editor history. An invalid scalar-splitting seeded NSRange cannot establish valid selection handling.

After a Rust main change, build the current FFI and bind the actual Swift link to it; a same-header old archive is insufficient. Keep producer/reviewer/own method sets separate (review28 is20 original+8, not48). Record the wrapper's source/object/link/image binding if selecting private tests outside the shipped manifest. Keep GUI/AX/live typing/keyboard undo/restart and Windows/Linux results UNRUN until separately executed. See `docs/implementation-progress.md`, UI56 source-character-count integration, for the actual publisher commands/results and limitations.

The status bar owns a single-entry `SourceCharacterCountCache` in SwiftUI `State`. Pass that same cache into new presentations, reuse counts when the full String is unchanged, invalidate on source changes (including equal-length edits), and return before cache access when disabled. Do not key counts to caret, selection, length alone or a global workspace cache. Test actual recount calls across recreated projections, preference transitions and grapheme-joining edits; these headless controls do not measure live typing latency.
