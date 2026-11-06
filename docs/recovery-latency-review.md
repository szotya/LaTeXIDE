# PR68 recovery and native editor latency — 2026-11-06

The recovery commit path and the complete native editor path are separate
measurements. The original agreed p95 limit of 16.7 ms was not met by the
packaged editor. A proposed 50 ms limit for ordinary 80-column LF source is
pending an explicit user decision. No failed measurement is reclassified.

## Measurement boundary

The isolated, ad-hoc signed app uses the normal SwiftUI application,
WorkspaceView, SourceTextView, WorkspaceStore, NativeRuntime, public C ABI and
shared Rust runtime. Its production editor code is unmodified by the driver.
Local instrumentation starts the driver, reads recovery status and records
timestamps. It uses ephemeral preferences, avoids application activation, and
sends local insertText calls to its own editor. It never sends global input,
uses a user's open project, connects a model or changes TCC.

Each sample replaces one UTF-16 character at the beginning of the file. The
first timestamp precedes insertText; the final timestamp follows the next
main-actor turn and own-window layout/displayIfNeeded. This is an AppKit display
boundary, not a hardware input-to-photon or release-performance qualification.
The 40-sample p50/p95/max select sorted samples 20/38/40, without trimming.
The normal debug Swift build and the packaged build's core profile are used.

Two workloads have exact physical sizes of 1 KiB and 1 MiB:

- Single-line ASCII exercises an extreme paragraph at the file ceiling.
- 80-byte LF lines contain TeX comments, with a bounded final partial line.

Each durable run requires recovery.enabled=true, successful edits and matching
editor/runtime text. It drops the runtime, reopens, verifies dirty text/revision
and recovered undo, and verifies that disk bytes did not change without Save.

## Retained observations

Times below are milliseconds. Diagnostic six-sample values do not qualify the
40-sample gate. The profiled run is retained separately and is not timing proof.

| Candidate / workload | Samples per size | 1 KiB p95 | 1 MiB p95 | Scope |
| --- | ---: | ---: | ---: | --- |
| 16db0f1, original UI, single line | 40 | 18.235 | 783.306 | FAIL against original 16.7 ms target; restore/undo/disk pass |
| First UI index/layout candidate, single line | 6 | 17.343 | 588.495 | Diagnostic; target not met |
| Private soft-wrap experiment, single line | 6 | 19.882 | 163.170 | Diagnostic only; wrapping not applied to product |
| First UI candidate, 80-column LF | 40 | 16.118 | 26.947 | Original target not met at 1 MiB; proposed criterion still pending |
| Private memory-only control, single line | 6 | 17.712 | 608.223 | Diagnostic; no recovery/restore/undo qualification |
| Final contiguous-layout candidate, single line | 40 | 45.666 | 510.243 | Original target FAIL; restore/undo/disk pass |
| Same final binary, 80-column LF | 40 | 33.783 | 30.497 | Original target FAIL; meets proposed 50 ms numerically, decision pending |

The memory control differs by one private factory call, opening Workspace
explicitly in memory. Its archive/source identity is separate. It verifies
recovery.enabled=false and unchanged disk, and makes no persistence claim.
Its long-line cost shows that the existing TextKit paragraph/layout work remains
large even without recovery. The diagnostic samples do not establish an exact
subtracted recovery latency or a statistical regression bound.

The ABI-correct production archive is
`5b4417cbf11ed36f7c23275107d1d1260868e83a0592fc25d109ef6bc455a4db`.
A separate native adapter/C ABI/lock run with 40 samples per size had p95
4.594 / 5.728 ms, with actual recovery/undo/disk assertions. Additional raw
suite timings are retained; these descriptive numbers are not UI acceptance.

## Editor changes and regression boundary

Ordinary accepted native edits update cached UTF-16 line offsets using the edit
range and inserted line breaks. Same-length edits without line breaks do not
rescan the document. Invalid ranges leave the cache unchanged. A stale cache,
failed edit, composition finalization or external history refresh rebuilds it.

Syntax coloring and the line-number ruler use the already laid-out visible glyph
range after requesting visible layout. They do not query total numberOfGlyphs
on each redraw. Selection/color invalidation marks the visible rectangle with
avoidAdditionalLayout=true; normal AppKit display performs the necessary layout.
See [Apple's display API contract](https://developer.apple.com/documentation/appkit/nstextview/setneedsdisplay(_:avoidadditionallayout:)).

The noncontiguous-layout experiment displaced restored viewport y850 to y2260.
The full regression caught two assertions in one method; that setting was
removed. Its failed 89-method run (84 method passes, 4 named skips, 1 failed
method with 2 assertions) remains separate from the final regression.
Horizontal scrolling, wrapping, text, UTF-16/scalar boundaries, composition,
selection and shared undo/redo contracts remain as before.

The new line-map tests compare incremental and freshly derived offsets across
Unicode, line insertion/deletion and all valid UTF-16 boundaries; a real native
editor test exercises newline/emoji edits and undo/redo synchronization.
The final full native regression has 89 methods: 85 PASS / 4 unchanged named
opt-in SKIP / 0 failures. It includes the three new line-map methods and the
existing viewport/selection/composition controls. Its separate ABI p95 is
6.995 / 8.095 ms, again descriptive. The final app executable SHA256 is
`92b041c620e5087aac1c3b56fb4b57ae0f07431a883a73a00a906287fd0cc88e`;
both final 40-sample workloads use exactly this binary. The fresh structure check passes all 195 Python tests plus generated/planning/
documentation validation. Current exact-head CI/review and the acceptance
decision remain separate merge gates.

## Custody and remaining qualification

Raw samples, own process identities, driver/production-source maps, archive/app
hashes, failed experiments and signed bundles remain under the publisher-owned
`pr68-review6-20261106` receipt. A separate T9 copy preserves logs/metadata when
the system volume ran out of space. No other cache or evidence was cleaned.
The memory-only and soft-wrap controls are excluded from production acceptance.

The large single-line paragraph remains an explicit performance limitation.
This work does not qualify Windows/Linux native behavior, hardware power-loss,
client receipt/exactly-once, accessibility, online models or full release use.
Protected merge still requires the agreed measurement boundary, current checks
and every relevant review thread resolved.
