# Definition Navigation Implementation Plan

> Implement inline using the existing authorized local workflow and superpowers:executing-plans; test first, one fresh final integrated review. The workspace has no Git repository; retain source/evidence/ledger, do not run Git-dependent helper/commit/cleanup steps.

**Goal:** Cmd+click own macros, ref labels and cite BibTeX keys to select their precise definitions.
**Architecture:** bounded Rust project index and additive revision-bound query; AppKit mouse hit testing, fresh target validation and native range navigation.
**Tech Stack:** existing Rust/Serde, Swift/AppKit/SwiftUI; no new dependency.
**Spec:** [definition navigation design](../specs/2026-10-31-definition-navigation-design.md).

## Constraints and review focus

Preserve UTF-16/scalar/revision/IME/focus/undo, 1 MiB files, 10 open buffers and existing index budgets. Keep user dirty work and unrelated audit artifacts intact. Review boundaries: exact-token versus prefix/delimiter hits; BOM/CRLF and Unicode target coordinates; dirty or moved target between resolve/open; click in blank layout space; navigation-only state versus shared undo.

## 1. Shared exact definitions

- [x] Add `tests/definition_contract.rs` against real workspaces/files: exact full macro/ref/cite lookup and offsets, lists/options/ignored contexts, precedence, dirty undo, unchanged-metadata disk refresh, BOM/CRLF, nonmutation and bounds. Run `bash scripts/cargo-local.sh test -p latexide-core --test definition_contract --offline`; expect missing API RED.
- [x] Add `src/definition.rs`, export it, and `Workspace.definition(id, expected_revision, offset_utf16) -> Result<DefinitionSet>`. Add exact target ranges to ProjectIndex symbols with monotonic UTF-16 conversion and normalized disk decoding. Reuse completion context only for lexical ownership, with exact full-token matching. Run target and full workspace tests; expect PASS.
- [x] Add `GetDefinition` to protocol/runtime, returning source id/revision and serialized DefinitionSet. Add a real IPC regression for dirty target/exact range/nonmutation/stale revision; observe unknown-command RED before wiring. Build FFI and run IPC tests; expect PASS.

## 2. Native Cmd+click

- [x] Add `DefinitionNavigationTests.swift`: real native editor/layout/mouse events resolve all kinds, select exact target and preserve source state/history; dirty target, scalar/CRLF, blank space/modifiers/IME/stale guards. Run filtered suite before implementation; expect missing capability RED.
- [x] Add a focused native store extension/DTO for fresh resolve → read target → re-resolve → expected-revision load → range navigation. Extend EditorNavigation with optional range/revision, preserving existing line navigation. Use SourceTextView glyph hit testing and exact Cmd modifier, dismiss old completion and keep source focus. Use compact actual navigation feedback for missing/partial/multiple matches. Run filtered and full native suites; expect PASS.

## 3. Delivery and maintained context

- [x] Build/sign with `bash scripts/build_macos.sh`; check shared MSVC x64 with the canonical offline cargo command. Run real Cmd mouse workflow in isolated native windows; preserve running user app/dirty buffers. Record any live-app check as separately observed/unrun.
- [x] One fresh read-only integrated review of implementation/spec/ledger; grade findings by effect, fix material ones in one RED→GREEN pass, run affected full suites. No repeated review.
- [x] Add `docs/definition-navigation-verification.md`; update current handoff, implementation ledger, README/product direction and reusable skill references. Validate skill/relative links, bundle hashes/signature and source freshness. Keep prior reports historical; Windows/Linux native remains unrun.

Final: Rust69PASS; native73executed/69PASS/4opt-in skips/0failures; IPC10/MCP1, shared MSVC x64 check and final staged arm64 build PASS. One review, all3findings reproduced/fixed in one pass, no deferred Minor. Formal skill valid;42Markdown/283relative links/0broken;4packaged hashes/palette/core stamp/signature consistent. Isolated native mouse/selection workflow verified; running user session preserved, new packaged-app live Cmd-click unrun. Detailed evidence: docs/definition-navigation-verification.md and docs/definition-navigation-review.md.
