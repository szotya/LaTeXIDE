---
name: latexide-platform-development
description: Use when developing this LaTeX IDE's shared Rust core, native macOS or Windows frontend, local MCP bridge, compiler integration, or a future Linux port, including onboarding from the Mac implementation.
---

# LaTeX IDE platform development

Use the accepted project contracts and current evidence when continuing work across platforms. Locate the repository root first; paths below are repository-relative, even if this skill has been copied into a personal skill directory.

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
| Parallel development packages, exclusive paths, contract gate and GitHub onboarding | [references/parallel-development.md](references/parallel-development.md) |
| Compiler acquisition, package roots and offline proof | `<repo>/tools/texlive/README.md` and `<repo>/docs/compiler-assessment.md` |

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
