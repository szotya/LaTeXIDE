# Native IDE intelligence implementation plan

> Use executing-plans inline for shared completion/native integration. The independent assistant files are delegated under dispatching-parallel-agents after the interface below is fixed. One fresh final integrated review, no per-task reviewers. Existing no-Git/local-evidence workflow remains; the user's choices authorize this work without another approval loop.

**Goal:** verified keyboard history, native command completion and a real Codex-backed native assistant panel.
**Architecture:** authoritative Rust document/history; pure Rust completion; native AppKit popup; isolated Swift assistant session/process and shared-palette inspector.
**Tech:** existing pinned Rust/Swift/AppKit/SwiftUI stack, local Codex CLI app-server stdio; no new UI dependency.
**Spec:** `docs/superpowers/specs/2026-10-31-ide-intelligence-design.md`.

## Constraints and review focus

- Preserve shared undo after save, UTF-16/revisions, IME and existing user projects.
- No physical-key shortcut alias; measured Hungarian layout must follow actual `z` character.
- Completion max20, scalar-safe replacement, no comment/verbatim/IME suggestions, stale acceptance discarded.
- Assistant uses actual project session/MCP, raw filesystem read-only, explicit save and no global config/auth mutation.
- Bound frames/transcript/process lifetime; project switch/cancel/EOF cannot leave stale replies in another workspace.
- Windows/Linux docs/skill maintained; no target execution claims.

## Tasks

1. Keyboard investigation — native key probe + actual MCP keyboard undo/redo, add layout characterization and record cause. Current live result is already positive; no production fix unless evidence requires it.
2. Shared completion — `crates/latexide-core/src/completion.rs`, workspace API, additive protocol/FFI `get_completions`; Rust/IPC tests RED→GREEN. Reply: `document_id,revision,replace_start_utf16,replace_length_utf16,prefix,items:[{label,detail,insert_text,selection_start_utf16,selection_length_utf16}]`.
3. Native completion — `Editor/CompletionController.swift` plus native representable hooks and persisted preference. Tests for actual insert/undo/selection/stale/IME; themed popup, keyboard navigation, rendered/live check.
4. Assistant independent boundary — create only `apps/macos/Sources/LaTeXIDEKit/Assistant/` and `Tests/LaTeXIDEKitTests/Assistant*`, optional protocol fixture files. `@MainActor AssistantSession: ObservableObject` with messages/state/error, `configure(root:URL?,sessionPath:String,bridgeURL:URL?)`, `connect(executable:URL)`, `send(text:String,context:String)`, `cancel()`, `disconnect()`, `newConversation()`. `AssistantPanel(session:AssistantSession,context:AssistantContext)` uses existing `ideTheme`; `AssistantContext` has projectName,documentID?,revision,selectionText,diagnosticsText. Agent may refine API only after messaging parent. No WorkspaceStore/View/App/preferences/shared-core/docs edits by this task. Parent integrates a PDF/Assistant inspector tab and project lifecycle. Test controlled real subprocess; no paid model turn or GUI interference.
5. Integrated UI — persistent inspector/tab/dimensions and completion preference; meaningful empty/error/running/cancel states; source focus/local field undo respected. Keep source usable at1080×680. Connect actual CLI handshake without generating a model reply.
6. Run relevant full suites/new regression/actual CLI handshake, staged package/signature and native render/live workflow. One fresh integrated review; fix material findings with reproductions. Update handoff/ledger/README/skill and exact implemented/partial/unrun status.

All code/test/evidence remains local. No Git initialization/commit/push/publication. Evidence roots: `evidence/keyboard-2026-10-31`, `evidence/intelligence-2026-10-31`.

## Final execution status — 2026-10-31

- [x] 1 Keyboard measured and actual saved-MCP Cmd+Z/redo verified. The later genuine focus bug was reproduced and fixed; no physical-key alias added.
- [x] 2 Shared completion6contracts and IPC dirty/revision/nonmutation contract PASS.
- [x] 3 Native popup insertion/selection/history/stale/IME/lifetime tests PASS; actual live Down+Tab/Cmd+Z/redo observed.
- [x] 4 Codex assistant controlled subprocess16active tests PASS and installed initialize-only handshake1PASS. Independent implementer did not initiate a model request.
- [x] 5 Parent integrated durable inspector/dimensions/completion preference and session-owned draft/attachments; actual PDF/hide retention and focused composer undo checked.
- [x] 6 One integrated review, all4findings fixed with RED→GREEN evidence. Actual parent live model check additionally exposed MCP approval failure; targeted regression and exact nine-tool grant fixed it. Actual read plus edit→Cmd+Z/redo→fresh build/PDFKit completed on own sample. Final package/tests/docs/skill closure is in the maintained verification/handoff.

Final suite: Rust35; native46executed/43pass/3opt-in skipped; separate native render2PASS and CLI initialization1PASS; IPC7, MCP1, compiler3; all five Rust crates MSVC x64 type-check. Exact scope, logs, current manifest and remaining qualification: [verification](../../intelligence-verification.md), [single review/fix record](../../intelligence-review.md).

Ruling: the parent performed one failed and two successful small actual model turns on the explicitly isolated own sample to verify real thread/MCP/edit behavior authorized by the user's selected assistant. No user project/private corpus was sent, no model/provider override or batch was added, and the implementer's no-model boundary was preserved. Cost: this is limited installed-alpha CLI/account evidence, not provider-wide production qualification. Windows/Linux native execution, release performance/accessibility and full semantic IDE scope remain open rather than falsely checked off here.
