# PR68 native latency implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task. The user's execution instruction is to continue in this chat; do not delegate to agents or create chats.

**Goal:** Meet the retained p95 <=16.7 ms packaged edit target at 1 KiB and 1 MiB, including a single line, before resolving F02 and merging PR68.

**Architecture:** Keep the complete native NSTextView text system and shared Rust authority. Bound supported long ASCII layout fragments horizontally on one visual row, retaining native fallback and logical navigation. Remove unconditional whole-document UTF-16 validation work.

**Tech stack:** Existing Swift/AppKit/SwiftUI/CoreText/Observation, XCTest and the shared Rust core; public C ABI and recovery wire format stay unchanged.

**Spec:** [Strict native latency design](../specs/2026-11-06-pr68-native-latency-design.md).

## Global constraints

- p95 <=16.7 ms; 40 samples per exact 1024/1048576-byte fixture, no trimming.
- Keep the original insert/next-main-turn/own-window layout/display boundary.
- Complete text storage, native input, horizontal scrolling and explicit Save; no synthetic characters or offset remapping.
- ASCII fixed-pitch fragments <=2048 glyphs; native fallback for unsupported input/attributes and composition.
- No new dependency, C ABI/lock/guard changes, agents, global input, TCC or userapp actions. Shared Rust changes are limited to byte-identical block comparisons in the existing recovery delta encoder; every durability barrier and ACK condition stays unchanged.
- Preserve failed attempts; no external T9 checkpoint retry. Fresh exact-head CI and all threads are merge gates.

## Review focus

- Caret movement, logical Home/End and selection spanning a fragment boundary.
- Font/style changes preserving native baseline, row height and scroll extent.
- Emoji, combining/RTL text, tabs/newlines and marked text selecting native fallback.
- Undo/redo, normalized CRLF and failed edits preserving authoritative text and cache agreement.
- Full-document native selection/clipboard and Find remaining independent of layout fragments.

### Task 1: Bound native paragraph geometry

**Files:** Create `apps/macos/Sources/LaTeXIDEKit/Editor/SourceTypesetter.swift`; modify `NativeEditor.swift` and, only if necessary for measured native behavior, `Editor/SourceTextView.swift`; create `apps/macos/Tests/LaTeXIDEKitTests/SourceTypesetterTests.swift`.

**Interfaces:** `SourceTypesetter: NSATSTypesetter`, overriding `layoutParagraph(at: UnsafeMutablePointer<NSPoint>) -> Int`; it owns support validation and font/style metric caching. NativeEditor installs it once on its existing layout manager.

- [x] Add a real-native failing test: a long ASCII row retains all text and one y coordinate while each glyph's fragment has length <=2048.
- [x] Run it against ef22b231 and retain the actual failure.
- [x] Implement supported-paragraph fragments with native line metrics; retain super.layoutParagraph for every unsupported case.
- [x] Test geometry at both sides of a boundary and the document end; font/paragraph changes; Unicode/tabs/marked text fallback; logical Home/End and selection extension; full private-pasteboard selection; native edit/undo/redo and unchanged disk.
- [x] Run targeted tests, correcting observed behavior without weakening assertions, then run the complete native suite.

### Task 2: Bound edit validation work

**Files:** Modify `apps/macos/Sources/LaTeXIDEKit/SourceEdit.swift` and `apps/macos/Tests/LaTeXIDEKitTests/TextEditTests.swift`.

**Interfaces:** Keep `SourceEdit.make(text:range:replacement:) throws -> SourceEdit` and its line/UTF-16 encoding unchanged.

- [x] Extend scalar/combining/newline/large-source and invalid-range controls around the existing failed performance reproduction.
- [x] Replace the full UTF-16 array with NSString length and boundary-code-unit checks; keep scalar-safe position conversion.
- [x] Verify all text-edit, CRLF/composition and shared-history tests.

### Task 3: Qualify the packaged candidate

**Files:** Update `docs/recovery-latency-review.md`, developer handoff, P01 results, implementation ledger and UI skill reference.

- [x] Build a distinct instrumented signed bundle from the final source and the qualified ABI-correct archive; preserve source and executable hashes.
- [x] Run both same-binary 40-sample workloads serially with fresh own fixtures and all recovery/undo/disk controls.
- [x] If the strict target fails, retain that result and profile only the owned process to identify the remaining cost; do not relabel it or resolve F02.
- [x] Record final local counts, metrics, profiles and failed attempts; fresh integrated17 PASS includes Python195/Rust331/native133. Exact-head hosted CI is Task4.

### Task 4: Publish and finish the existing PR

- [ ] Commit and push the reviewed fix normally; do not force or rerun failed CI manually.
- [ ] Address still-valid new review feedback and follow the then-current exact-head CI to completion.
- [ ] Resolve F02 only with the strict passing measurement and current source/test evidence; verify every thread is resolved.
- [ ] Refresh head/base/rules/checks/reviews and use the authorized protected expected-head squash merge; verify merged main and its post-merge CI.

## Measured continuation — 2026-11-07

The original fragments and validation changes did not meet the retained target. Owned profiles justified certified same-length ASCII glyph refresh, field-specific Observation with presentation metadata independent of revision churn, and synchronous native drawing into a pixel-aligned viewport surface. The bitmap uses the actual window color profile/BGRA format; only complete backing pixels in native display damage are repainted, and earlier committed images remain immutable. Unsupported shaping and composition continue through native TextKit. The same NSApplication/window owns all input and drawing; no asynchronous presentation is used.

The remaining large-file cost also included bytewise comparisons of unchanged recovery text. The shared delta encoder now compares equal blocks before the unchanged scalar-boundary trims. An exact legacy-byte/round-trip test covers empty, combining, emoji, block-boundary and 1 MiB cases. A fresh archive is required; the previous unchanged-archive design assumption is superseded by this measured continuation, within the user's retained-target/larger-fix instruction. No persistence format, workflow or flush/ACK guard changes are part of it. The single F05 runtime_delta source hash/size must be rebound to the qualified changed source; the six other source pins and strict source/mode/nonce/receipt equality guards stay fixed, and fresh F05 qualification is required.

Actual hosted lifecycle tests found initial updateNSView before window attachment/viewport sizing. Guarded restoration now waits for that main-turn layout and yields to explicit native input/navigation. Pixel tests found fractional highlight damage; outward backing-pixel alignment fixes it. A pre-existing keyboard fixture manually ended an event-owned undo group; the fixture now owns an explicit group, with its original undo/redo assertions retained.

Latest local native qualification: 133 methods =129 PASS +4 unchanged opt-in SKIP, 0 FAIL, 17.682 s. Candidate39 same-binary strict40-sample UI p95 passes all four cases: single13.491/16.109 ms, LF14.129/15.832 ms. Every actual-image/recovery/undo/disk guard passes. Exact commands, source manifests and all failed controls remain under the private PR68 evidence root. Fresh complete integrated/F0517 PASS (Python195/Rust331/native129 PASS+4 SKIP). Current-head remote CI/review remain pending.
