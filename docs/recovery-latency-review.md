# PR68 recovery and native editor latency — 2026-11-07

The retained p95 <=16.7 ms target now passes locally for all four cases, including
single-line 1 MiB. The proposed 50 ms limit was rejected. Candidate39 uses the
same signed executable for both layouts, the original 40 samples per size and
the original insert/next-main-turn/own-window layout/display clock. No samples
are trimmed. Current-head CI, review and protected merge are separate gates;
F02 remains open until the published fix and evidence are reviewed.

| Layout | Bytes | p50 ms | p95 ms | Max ms | p95 <=16.7 ms |
| --- | ---: | ---: | ---: | ---: | --- |
| Single line | 1024 | 11.098 | 13.491 | 14.795 | PASS |
| Single line | 1048576 | 13.423 | 16.109 | 17.368 | PASS |
| 80-column LF | 1024 | 11.632 | 14.129 | 27.004 | PASS |
| 80-column LF | 1048576 | 14.528 | 15.832 | 16.796 | PASS |

Every case verifies actual native viewport image publication after each edit,
literal editor/core text agreement, recovery enabled, revision/dirty state,
actual runtime drop/reopen, recovered undo and unchanged disk without Save.
The target is p95; the larger maxima above are retained. This is a controlled
own-window AppKit display measurement, not hardware input-to-photon or broad
release-performance acceptance.

Evidence identities (SHA256):

- Executable: `ba15ffbb93424d9cb79ef0f031b46b6225c0b8e0801b2d07adcda4cec2bd3530`.
- Compiled source manifest: `6c39893c1569127c632a80a579f3786cb00753dc80ac8b10081eaf0a86e9254b`.
- New Rust archive: `1abc94f4a59412bad28aa244c6cca1c247901bf1a480941be6fd79a4089817e1`.
- Single-line report: `7b592855afbbb4fa4c13deb37ace428a4894ea70c2f5a62bbf7db34cd63c9f4b`.
- LF report: `fe2311aebf1d51c5f574a1ab110125c2b6b10fed3488bbb208f4f332e9a8aaaf`.

Raw samples, signed bundles, original/compiled source maps, commands, owned PIDs
and all failed attempts remain under the private `pr68-review6-20261106` evidence
root. `metadata/strict-all-four-qualification.json` binds the final reports and
native test manifest. Product inputs match the working tree; measurement-only
changes are the own-app entry, ephemeral preferences, read-only status accessor
and driver. No global input, activation, user project, TCC or model call is used.
The refused later external T9 checkpoint transfer was not retried.

## Native continuation — 2026-11-07

Supported printable ASCII paragraphs retain complete native text storage and
native glyphs. Fixed-pitch metrics are validated against AppKit and all printable
ASCII advances; horizontal fragments contain at most 2048 glyphs on the same
visual row. Logical Home/End, selection, clipboard and line queries keep source
positions. Unsupported attributes, Unicode, tabs, shaping and marked text use
native processing. Noncontiguous layout is now conditional on a complete,
validated single row; ordinary multiline viewport restoration remains native.

A complete uniform system-monospaced row of at least 1024 UTF-16 units can retain
its already validated geometry for bounded, same-length printable-ASCII edits.
Only changed glyph IDs and the native elastic-space property are replaced.
Length, font/style, unsupported attributes, text or composition invalidate the
certificate and use AppKit. Tests compare glyph IDs/properties/character indices
and geometry with a separate native manager, including another manager sharing
the same text storage. No synthetic characters or position mapping is introduced.

Literal UTF-16 guards replace canonical String equality in synchronization,
coloring, completion and navigation. SourceEdit checks only length and scalar
boundaries; normalized replacement handling stays aligned with the core. Syntax
foreground colors use layout-manager temporary attributes instead of source
attribute edits. Complete source validation rejects a bounded prefix containing
newlines before making a full ASCII conversion.

Field-specific Observation and revision-free tab presentation prevent unrelated
SwiftUI work on every acknowledged edit. Revision-bearing core metadata stays
authoritative; saves still update dirty presentation without a revision change.
Real NSHostingView tests exercise external NFC/NFD edits, the next UTF-16 edit
and undo, as well as destruction/recreation restoring selection and scroll.
Initial zero-sized, unattached representables restore after hosting layout;
explicit native input/navigation cancels the deferred restoration.

Native drawing publishes synchronously into an owned viewport-sized bitmap using
the actual window color profile and BGRA format. Damage aligns outward to backing
pixels; prior CGImages remain immutable. Certified single-row surfaces cover the
row and font overhang, with the native background covering the rest. Idle updates
reuse the image; explicit full invalidation still repaints. Repeated unchanged
caret notifications no longer force an extra whole-viewport redraw. Actual layer
composites and fresh native draws match across scrolling, selection, theme,
Unicode/composition, rejected edits and long-row fragment boundaries.

The shared recovery delta encoder compares equal 4096-byte blocks, then uses the
unchanged exact byte/scalar-boundary logic. A legacy-byte/round-trip regression
covers empty, emoji, combining, block boundaries and front/middle/end 1 MiB edits.
Wire bytes, C ABI, Cargo profiles/lock, all barriers, final full-media flush and
acknowledgment conditions stay unchanged. Only the changed runtime_delta source
SHA/size is rebound in F05_SOURCE_PINS; mode 420, the other six pins and every
source/mode/nonce/receipt guard are preserved. Fresh F05 qualification passes in the complete integrated run below.

Full final-source native verification: **133 methods, 129 PASS, four unchanged
named opt-in SKIP, zero failures**, 17.682 s. Earlier direct regressions reproduced
full-row glyph regeneration, canonical-text drift, stale fractional pixels,
hosted scroll restoration, delayed image publication and redundant redraws before
fixes. A keyboard test now owns its explicit UndoManager group instead of ending
an event-owned group; its original undo/redo assertions remain. The separate ABI
latency in that suite is descriptive (p95 6.345/5.934 ms), not the UI gate.
Raw default offline Rust workspace verification passed 327 methods with six
ignored; it is not the complete integrated parent-cohort qualification. Python
contracts passed 195 after the exact F05 source rebind. Fresh full integrated results are recorded below; current remote CI/review
remain separate publication gates.

[Design](superpowers/specs/2026-11-06-pr68-native-latency-design.md) and
[implementation plan](superpowers/plans/2026-11-06-pr68-native-latency.md).

## Retained strict continuation attempts

All values are p95 milliseconds at 1 KiB/1 MiB, 40 samples per size unless the
entry explicitly says diagnostic/error/unrun. Every failed result remains FAIL.
Early private prototypes do not establish product acceptance.

| Attempt | Workload | p95 1 KiB / 1 MiB | Result |
| --- | --- | --- | --- |
| 1 fragments | single | 33.835 / 75.255 | FAIL |
| 2 literal guards | single | 36.258 / 68.944 | FAIL |
| 3 drawing-only colors | single | 34.735 / 84.145 | FAIL |
| 5 private Observation | single | 20.709 / 81.738 | FAIL |
| 6 visible glyph drawing | single | 22.815 / 69.776 | FAIL |
| 8 narrow request | single | 44.079 / 57.153 | FAIL; rejected |
| 9 direct glyph request | single | 22.330 / 69.212 | FAIL |
| 10 nominal generation | single | 26.129 / 68.654 | FAIL; rejected |
| 12 certified geometry | single | 28.038 / 37.564 | FAIL |
| 13 trace | single | 25.178 / 46.556 | FAIL |
| 14 metadata projection | single | 25.024 / 33.696 | FAIL |
| 15 readonly presentation | single | 23.531 / 29.854 | FAIL |
| 16 opaque background | single | 24.034 / 32.519 | FAIL |
| 17 layer policy controls | single | 20.016 / 30.527; 20.406 / 31.211 | FAIL; oversized geometry rejected |
| 18 fixed size | single | 20.964 / 32.487 | FAIL; rejected |
| 19 opaque native fill | single | 21.713 / 33.613 | FAIL |
| 20 outer clipping control | single | 18.943 / 30.430 | FAIL; clipping removal rejected |
| 21 damage diagnostics | single | — | ERROR before measurement; own diagnostic overflow |
| 22 finite damage diagnostics | single | 18.797 / 31.984 | FAIL |
| 23 redraw policy | single | 19.461 / 30.145 | FAIL |
| 24 layer-tree diagnostic | single | — | Compile ERROR, zero launches |
| 25 layer-tree diagnostic | single | 23.638 / 30.179 | FAIL |
| 26 custom source layer | single | — | Built, UNRUN; potentially oversized bitmap rejected |
| 27 scroll-layer control | single | 19.447 / 29.948 | FAIL |
| 28 clip-layer control | single | 21.288 / 29.961 | FAIL; earlier ENOSPC attempt invalid |
| 29 own viewport bitmap | single | 28.305 / 31.568 | FAIL |
| 30 window-profile BGRA | single | 15.859 / 18.392 | FAIL |
| 31 certified drawing | single; LF | 15.820 / 15.850; 15.222 / 29.909 | Single PASS, LF FAIL; not all-four acceptance |
| 32 bounded ASCII rejection | LF | 15.559 / 19.451 | FAIL |
| 33 persistent damage buffer | LF | 20.030 / 18.990 | FAIL |
| 34 product preparation | — | — | Compile ERROR, zero launches; wrong diagnostic driver |
| 35 product | single | — | Image-publication guard FAIL before completed measurements |
| 36 forced displays | single | 27.482 / 23.021 | FAIL |
| 37 coalesced producer | single | 18.015 / 17.422 | FAIL; LF UNRUN |
| 38 1 KiB certificate | single; LF | 13.728 / 15.977; 16.970 / 19.938 | Single PASS, LF FAIL |
| 39 unchanged-caret redraw | single; LF | 13.491 / 16.109; 14.129 / 15.832 | All four PASS; identities above |

Private preparation-only controls without a completed measured cohort and
owned-process sampled diagnostics remain separately marked in the raw ledger;
they do not contribute passing samples. The frozen c17 SIGABRT/zero-case/four-UNRUN
attempt was never replayed. No failed receipt or old green CI qualifies new code.

## Complete local integrated qualification — 2026-11-07

Fresh private source/target/fixture qualification passes all **17 required steps**.
Python195 PASS; Rust329 normal +2 exact parents =331 framework PASS, zero
failures/ignored, four normal filtered. The32+5 nested cases are separate;
child PASS remains0. Seam3, real P18 PDF4, tool acquisition6, IPC10, MCP1,
native schema1 and native133 (129 PASS/four unchanged named opt-in SKIP)
pass. Runtime-clean rebuilt all selected packages before the bound cohort.
Fresh F05 proves the exact rebound encoder source under the unchanged guards.

Integrated source SHA256: `a6685a87d71f1ec1ee8930a76a578fbff16aeae79d8d4fd966d5acf8a883af36`.
Parent/cohort proof SHA256: `69451071f8b51debb7051b41edd7221a68c930d07730d25eef8581bdc641da10`.
Report SHA256: `7733340596db04e7f2e480d8718c137f7d2f76104abda24decd227501004aee3`.
The source snapshot stayed byte-stable. Later documentation records this result;
product and verifier code stay identical. Fresh exact-head hosted CI/review are
still required for publication/merge. The first untagged-cache attempt correctly
failed runtime-clean and left dependent controls UNRUN; its report is preserved.
The succeeding fresh attempt used an existing owned Cargo-created cache tag and
kept the required clean step, with no fabricated marker or guard bypass.

## Historical measurements through ef22b231

The observations below retain their original cutoff and then-pending criterion.
The 50 ms proposal was subsequently rejected, and the earlier approximately
608 ms memory-only result is a historical bottleneck observation, not an inherent
AppKit limit. Candidate39 above supersedes that performance limitation. The old
ef22b231 CI (7 jobs/17 steps, Python195/Rust330/native87 PASS+4 SKIP) qualifies
only that earlier source.

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
| App 92b041c6, single line | 40 | 45.666 | 510.243 | Original target FAIL; restore/undo/disk pass |
| Same app, 80-column LF | 40 | 33.783 | 30.497 | Original target FAIL; meets proposed 50 ms numerically, decision pending |
| CRLF/composition-corrected binary, single line | 40 | 32.896 | 519.369 | Original target FAIL; restore/undo/disk pass |
| Same corrected binary, 80-column LF | 40 | 33.874 | 37.136 | Original target FAIL; proposed 50 ms meets numerically, decision pending |
| App 3adcfae6, single line | 40 | 33.901 | 509.957 | Original target FAIL; restore/undo/disk pass |
| Same latest app, 80-column LF | 40 | 31.702 | 30.187 | Original target FAIL; proposed 50 ms meets numerically, decision pending |
| App 9aefca1b, single line | 40 | 32.856 | 513.615 | Original target FAIL; restore/undo/disk pass |
| Same shared-geometry app, 80-column LF | 40 | 34.216 | 32.177 | Original target FAIL; proposed 50 ms meets numerically, decision pending |

The memory control differs by one private factory call, opening Workspace
explicitly in memory. Its archive/source identity is separate. It verifies
recovery.enabled=false and unchanged disk, and makes no persistence claim.
Its long-line cost shows that the existing TextKit paragraph/layout work remains
large even without recovery. The diagnostic samples do not establish an exact
subtracted recovery latency or a statistical regression bound.

The historical ABI-correct archive was
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
The first full native regression has 89 methods: 85 PASS / 4 unchanged named
opt-in SKIP / 0 failures. It includes the three new line-map methods and the
existing viewport/selection/composition controls. Its separate ABI p95 is
6.995 / 8.095 ms, again descriptive. The first final app executable SHA256 is
`92b041c620e5087aac1c3b56fb4b57ae0f07431a883a73a00a906287fd0cc88e`;
both final 40-sample workloads use exactly this binary. The earlier structure check passes all 195 Python tests plus generated/planning/
documentation validation. Current exact-head CI/review and the acceptance
decision remain separate merge gates.

## CRLF, composition and older compiler follow-up

The a625f3b Actions run passed 195 Python and 330 Rust methods, but its older
Swift compiler could not type-check the new test's combined filter expression.
Native build was ERROR and native controls were UNRUN. The test now constructs
an explicitly typed UInt16/Int boundary list in simple steps, preserving every
case and assertion; no fixture, skip or coverage was removed.

Two new real-editor regressions reproduced nine assertions in two failed methods:
marked text changed displayed line starts before the core committed, and CRLF
insertion left raw editor offsets different from normalized store offsets.
The ruler now derives transient geometry from marked text while composition is
active. After accepted CRLF edits or composition, the coordinator aligns literal
UTF-16 editor text and caret with store.text, and updates line offsets from the
normalized replacement. Ordinary keystrokes inspect only the replacement length for normalization;
they do not compare or scan the whole document. The core remains unchanged during composition; its end
still creates exactly one undo transaction. Ordinary and marked CRLF/emoji
cases verify the next edit, literal UTF-16 agreement, caret and unchanged disk.

All five line-map methods pass. The new complete native regression has 91 methods:
87 PASS / 4 unchanged opt-in SKIP / 0 failures. Its separate ABI p95 is
7.014 / 7.018 ms. The corrected packaged executable SHA256 is
`84352204e76bafee607750793a775fb761edc6f213af369b6ed774dfb7c49330`;
both corrected 40-sample workloads above use this exact binary. Rust/C ABI and
its archive remain unchanged. The original 16.7 ms target remains failed;
the proposed ordinary-LF 50 ms criterion still needs an explicit user decision.

The final replacement-bounded path again passes all 91 native methods (87 PASS /
4 unchanged opt-in SKIP). Its separate descriptive ABI p95 is 7.038 / 7.036 ms.
The latest app SHA256 is
`3adcfae66f34bf24c15c5d781d71767c0f069b1470492b92d215d9100fbfbb4f`;
the latest two table rows use this identical binary with 40 samples per size.
The previous app rows remain historical observations, with their original hashes
and failed original target. Current CI and the proposed criterion are still pending.

The same transient index also serves the status caret. A new assertion reproduced
an incorrect marked-text column (4 instead of 3); the ruler and caret now share
the display geometry. The core still stays unchanged before the single commit.
The complete native regression is again 91 methods: 87 PASS / 4 unchanged skips,
0 failures (19.176 seconds). Its descriptive ABI p95 is 7.130 / 10.759 ms.
The latest shared-geometry app SHA256 is
`9aefca1b27ac55e3ed5b9b5de3e57d55475e536a8faab84febd570dec762fc39`;
its two table rows use the same binary and fresh 40-sample fixtures. The original
criterion remains failed and the proposed ordinary-source criterion is pending.

## Custody and remaining qualification

Raw samples, own process identities, driver/production-source maps, archive/app
hashes, failed experiments and signed bundles remain under the publisher-owned
`pr68-review6-20261106` receipt. A separate T9 copy preserves logs/metadata when
the system volume ran out of space. No other cache or evidence was cleaned.
The memory-only and soft-wrap controls are excluded from production acceptance.

The historical large-paragraph performance limitation was addressed by the
strict continuation above; these old failed observations are not relabeled.
This work does not qualify Windows/Linux native behavior, hardware power-loss,
client receipt/exactly-once, accessibility, online models or full release use.
Protected merge still requires the agreed measurement boundary, current checks
and every relevant review thread resolved.
