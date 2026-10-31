# Developer IDE UI Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for the Mac/shared work with one fresh final review. The independent Windows source adapter is delegated under dispatching-parallel-agents after the shared contracts are fixed. Preserve the existing no-Git/local-evidence workflow and the user's authorization; no new approval round for the requested reversible UI refinement.

**Goal:** Deliver a premium native developer workspace with real IDE navigation, multiple system/fixed palettes and shared LaTeX syntax coloring.

**Architecture:** Rust remains the sole text/history authority and adds a compact lexical syntax-window response. Native presentation state owns tabs, panels and theme preferences. Shared local JSON defines palette families. AppKit/PDFKit and WinUI remain the actual native content controls.

**Tech:** Current pinned Rust/SwiftPM/C++/WinRT stack; no third-party UI runtime or downloaded editor.

**Spec:** `docs/superpowers/specs/2026-10-30-developer-ui-design.md`.

## Global constraints

- Preserve UTF-16/revision/IME/save/undo/MCP contracts; no per-edit approval.
- No source per keystroke JSON; max syntax window32768 UTF-16, bounded encoded reply.
- Palette families Graphite/Ocean/Dune/High Contrast × light/dark; appearance System/Light/Dark.
- Native UI only, no fake features/status, no user-file overwrite, Windows runtime unrun until a runner exists.
- Maintain central handoff, ledger and project skill references after material phases.

## Review focus

- Escaped percent, emoji, multiline math and verbatim must not corrupt token ranges or text (Task1 tests).
- Applying a theme/highlight must not increment revision/undo or interrupt composition (Task2 native tests).
- Tab changes/visual close must retain unsaved content and shared undo; metadata-only saves refresh dirty markers (Task3 tests).
- Keyboard/focus navigation, long paths and small windows must remain usable; controls must act (Task3 live inspection).
- Windows appearance/highlight source must follow the shared contract without claiming runtime verification (Task4 source checks).

**Status2026-10-31:** implementation, one final review/fix pass, suites and documentation closure are complete. Remaining live capture/hardware and target-platform qualifications are explicit below; no Windows native result is claimed.

## Task1 — Shared syntax and palette contract

Files: core `syntax.rs`, protocol/FFI syntax-window request; `shared/ide-theme/` local resource package; `ThemeCatalog.swift`; Rust/palette tests.

- [x] Write/run failing Rust syntax tests for escaped comments, math/env/verbatim/UTF-16/window boundaries and non-mutating runtime request.
- [x] Implement single-pass bounded lexical spans and revision-protected native request.
- [x] Write/run palette contrast/mode/persistence tests; add shared JSON resource package and typed theme catalog.
- [x] Run current Rust suite and targeted Swift tests. Expected: correct stable ranges, unchanged source/revision, readable colors and mode selection.

## Task2 — Native source editor

Files: `NativeEditor.swift`, source text/gutter/coloring helpers; native integration tests.

- [x] Test color-only updates, marked text and core history using real native controls/runtime.
- [x] Add visible-range debounced Rust coloring, current line, native gutter, native Find and caret reporting.
- [x] Verify new/old native tests. Expected: source bytes/revision unchanged on recolor; composition and undo still work.

## Task3 — Developer workspace

Files: WorkspaceStore presentation extensions, focused SwiftUI components, App scenes/commands/settings.

- [x] Verify retained tabs/history with native tests; implement and independently review revision-independent dirty metadata refresh.
- [x] Build compact chrome/activity/project tree/tabs/breadcrumb/code+PDF/bottom panels/status, theme popover and real Quick Open/commands.
- [x] Build/package and inspect controlled native dark/light layouts and syntax; observe live project/Quick Open/Find/palettes/PDF. Final app has live MCP/native-menu undo/save/toolbar compile with runtime PDF proof. Exact scopes: `docs/ui-verification.md`.
- [ ] Remaining qualification: final post-build PDFKit observation/readable live capture (ScreenCaptureKit -3812), hardware shortcut/IME, OS theme changes, resize extremes/accessibility. These are not inferred from tests or controlled renders.

## Task4 — Windows source and reusable handoff

Files: WinUI XAML/source, staging shared palette JSON, project skill references.

- [x] Carry shared theme/palette/syntax contract into native Windows source and add compact layout regions.
- [x] Check current Rust MSVC target and XML/source/portable helpers. Expected: source checks, no fabricated native Windows result.
- [x] Update handoff/ledger/theme contract references for Windows and future Linux.

## Task5 — Final verification and independent review

- [x] Run applicable full suites, final Mac build/signature check, controlled native artifacts and available live workflow. On resume, confirm package/source integrity; do not repeat unchanged suites. Final PDF-display/hardware qualification remains as listed above.
- [x] One fresh read-only whole-change review; both P2 findings fixed. Native selection/viewport regression RED→GREEN and full native suite15pass; Windows main-file choice source fixed and portable helper passed, target runtime unrun. No second review.
- [x] Record implemented/partial/unrun states and actual run commands/artifacts in README/handoff/skill. Preserve all local work; no Git initialization, commit, push or deployment.
