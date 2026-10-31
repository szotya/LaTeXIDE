# Project Completion Implementation Plan

> Execute inline with superpowers:executing-plans; test-first, one fresh final integrated review. Existing user authorization covers this extension; no Git initialization/commit/publication or extra design-approval loop. Preserve local evidence.

**Goal:** own macros, reference labels and BibTeX keys in the current native completion UI.
**Architecture:** cached bounded Rust lexical index, authoritative open-buffer overlay, existing additive IPC query/native popup; force-refresh validation on acceptance.
**Tech:** pinned Rust/Serde and Swift/AppKit/SwiftUI, no new dependency.
**Spec:** [project-completion design](../specs/2026-10-31-project-completion-design.md).

## Constraints and review focus

Preserve UTF-16/scalar/revision/IME/focus/history,20 candidates,1MiB/file,10 retained documents. Index budgets:128files/8MiB/8192symbols/10000visited entries. No TeX execution, global configuration changes or unrelated user files. Mac native only; MSVC check is not Windows execution.

Review deliberately: cross-file changed definitions with unchanged active revision; warm-cache forced refresh and disk errors/removal; malformed nested BibTeX/TeX at document limits; scalar-safe list suffix replacement; partial-index visibility and cold/warm main-thread cost. Each is exercised below.

## Tasks

### 1 Shared project context

- Create `crates/latexide-core/src/tex_scan.rs` for bounded borrowed lexical commands/groups, `project_index.rs` for source discovery/cache/symbols and BibTeX parsing.
- Modify completion/workspace/lib for typed project candidates and read-only query integration. Workspace `completions(&mut self,id,revision,caret)` preserves current call shape; `refresh_completion_index()` requests a fresh disk pass.
- [x] Write/run `tests/project_completion_contract.rs` against the real workspace/filesystem: own macros/arguments, dirty overlay+undo, ref list/Unicode range, cite options/list/entry types, ignored contexts, signatures/duplicates, nonmutation/editor budget, malformed/bounds/symlink and forced refresh. Observe missing-feature failures before implementation.
- [x] Implement bounded lexical parsing/cache and argument context; run full Rust suite. Record cold/warm timing on an own generated corpus without a brittle performance threshold.

### 2 Actual IPC and stale native acceptance

- Modify `crates/latexide-protocol/src/lib.rs` and `latexide-ffi/src/runtime.rs`: optional `refresh_project` bool default false; additive item kind/index metadata.
- [x] Add/run actual IPC regressions for dirty cross-file completion and changed disk content under forced refresh, preserving active revision/history.
- Modify `WorkspacePresentation.swift`, WorkspaceStore completion method and `Editor/CompletionController.swift` to decode metadata and freshly validate a selected candidate before insertion; stale key returns false.
- [x] Add native RED→GREEN tests for ref/cite/macro insertion/caret/one undo, and changed other-file definition rejected without consuming Enter/Tab. Run full native suite.

### 3 Coherent candidate presentation

- [x] Extend existing popup with compact kind badge, provenance and partial-index hint. Add opt-in actual native component renders for macro/ref/cite; inspect and perform live typing/keyboard checks on an own separate project. No fake assistant/model work.
- [x] Build/sign the final arm64 app and rerun relevant contracts plus MSVC x64 type-check. One fresh integrated review; material fixes get reproductions/full affected suites, no re-review.

### 4 Maintained handoff

- [x] Record exact results/limits/review/rulings in a new verification document and ledger; update handoff/README/product capability matrix.
- [x] Extend project skill references for index/dirty overlay/acceptance/platform behavior; validate structure/relative links and final bundle manifest. Keep current and historical phase evidence distinct.


## Final results

Shared Rust56PASS, native57executed/53PASS/4opt-in skips, separate project render1PASS (three actual popups), IPC9/MCP1/compiler3 and MSVC x64 shared check PASS. One fresh review, all3findings fixed with RED→GREEN; exact-key/no-reopen native acceptance regressions fixed in the same pass. Actual own-project macro/ref/cite, dirty cross-file agent label, keyboard undo/redo and successful fresh PDFKit observed. Current evidence/remaining boundaries: [verification](../../project-completion-verification.md), [review dispositions](../../project-completion-review.md). No native Windows/Linux run or full semantic/release qualification is inferred.

Rulings: retain the already-authorized inline/local/no-Git workflow; use bounded lexical Rust indexing rather than introduce a full language server; optimize the dev core only after measured cold latency (stepping tradeoff); keep explicit open-buffer lexical precedence rather than infer TeX include scope; repair missing closers only on final list segments. See the review/ledger for every scoped decision and its cost.
