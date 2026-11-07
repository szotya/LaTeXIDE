# PR68 strict native edit latency

The user retained p95 <=16.7 ms, including the single-line 1 MiB workload,
and explicitly requested the larger native editor fix. The proposed 50 ms
ordinary-source criterion was not accepted. F02 remains open until the strict
measurement and then-current publication gates pass. Earlier failed samples
remain failed.

## Evidence and implementation choice

At ef22b231, the packaged app's single-line 1 MiB insert/display p95 was
417.145/513.615 ms. A separate stock AppKit control reproduces the cost without
recovery. TextKit 2 makes this paragraph slower (about 2.6 seconds); replacing
only the typesetter still leaves about 380 ms. An owned-process profile places
most remaining time in insertion-point geometry over the entire glyph line.
Bounded fragments on the same horizontal row reduce stock input to about
19 ms. These diagnostic controls do not qualify the packaged-app gate.

Keep NSTextView and its complete text storage, selection, native input,
clipboard, Find and accessibility interfaces. For supported long, fixed-pitch
ASCII paragraphs, use at most 2048 glyphs per layout fragment on the same row.
No soft wrap, synthetic newline, truncated document or offset remapping is
introduced. All other paragraphs retain AppKit's typesetter. This is the
implementer's choice within the user's authorized native-editor fix.

The fast path must reject complex text, incompatible paragraph/font attributes,
marked text and non-1:1 glyph mappings. Font/style changes must invalidate its
metrics. Natural native line height/baseline, horizontal scrolling and logical
line navigation must survive fragment boundaries. Narrowing AppKit's edited
range was ineffective in a private control and will not be shipped.

Separately remove the unconditional full-document UTF-16 array allocation from
SourceEdit.make. Check only the two scalar boundaries before computing the
same logical line/UTF-16 positions. Preserve revision checks, CRLF normalization,
composition, recovery publication and shared undo/redo.

## Acceptance and scope

Use the same own signed normal SwiftUI app, SourceTextView/WorkspaceStore/native
adapter/C ABI/runtime-lock path, exact 1024/1048576-byte fixtures, 40 samples per
size and sorted 38th-sample p95. Timing remains before local insertText through
the next main-actor turn and own-window layout/displayIfNeeded. Exercise both
single-line and ordinary 80-column LF input. Require enabled recovery, successful
edits, literal editor/core agreement, actual runtime drop/reopen, dirty/revision,
recovered undo and unchanged disk. This remains an AppKit display boundary,
not hardware input-to-photon or full release-performance acceptance.

Preserve Unicode/IME/CRLF, native attributes, viewport and selection tests.
No global input, CUA, TCC, live userapp, foreign process inspection, online model,
new agent, dependency, Cargo.lock change or guard/protection weakening. Keep
evidence under the owned local scratch root; the refused checkpoint transfer
to T9 is not retried. Merge remains conditional on all actual current gates.

## Measured continuation — 2026-11-07

The implementation plan records the evidence-driven extension to a guarded glyph cache, field-specific Observation, synchronous window-profile viewport drawing and byte-identical shared recovery delta block comparisons. The original unchanged-Rust-archive assumption is replaced by a newly built archive and full Rust/native qualification. Native text authority, public C ABI, persistence format, all durability barriers and the original 40-sample 16.7 ms clock remain fixed. Only the exact F05 runtime_delta source hash/size binding changes; all six other source pins, guard logic, workflow and lock remain fixed, and fresh F05 qualification is required.
