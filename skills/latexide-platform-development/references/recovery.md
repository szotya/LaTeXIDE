# Single-Workspace durable recovery

Read `crates/latexide-core/src/services/recovery/RUNTIME.md`, `FORMAT.md` and the latest P01 entry in `docs/development/results/P01.md`. The private producer format and runtime companion formats are independent of frozen contract-v1, authenticated IPC v1 and the C ABI.

- Keep the one authoritative `Workspace`; persist rollback/recovery values, not a second runtime. Mac FFI `Runtime::open_project` enables `<project>/.latexide/recovery` before sharing the same runtime with native/agent callers. `Workspace::open_recoverable` is the core entry; plain `open` remains explicitly in memory.
- Persist saved text, physical BOM/line-ending metadata, saved file identity, origin/transaction, and both undo/redo stacks. Verify full-byte hashes and provider/state agreement; validate history ranges/text before making them live. Never derive a Save payload or overwrite permission from a decoded shared DTO.
- Before edit/open/undo/redo success, persist the complete candidate. Before selector publication failures roll memory back. After rename with unconfirmed sync/observer completion, retain the complete candidate, return `recovery_commit_uncertain`, block mutations and reopen. Local publication is not proof that a client received a reply.
- Save is explicit and keeps history. Commit a Save intent before the project-file write and finalize the new saved baseline afterward. If disk Save succeeds but recovery finalization fails, report `save_completed_recovery_failed`. Reconcile a pending intent only with bounded, nofollow, canonical in-project bytes; restore never writes documents. Disk replacement/content conflict must still prevent Save.
- Keep current/previous cohorts, bounded metadata and history. Prune only recognized, unselected private runtime artifacts after commitment; reject unknown/symlink/hardlink/permissive entries. Earlier developer deliveries/evidence are never runtime cache targets. Preserve the original 1 MiB/file, ten-buffer and history ceilings.
- Mac uses a private descriptor-held nonblocking flock. No PID scanning/stale-lock deletion. Other storage platforms return unconfirmed; the Windows factory remains in memory until its own adapter is implemented and tested. Do not claim Windows/Linux/GUI/power-loss qualification from Mac Rust tests.

Run the relevant core and FFI Rust tests plus the full Rust workspace after shared hook changes. Tests can use normal TMPDIR; explicit P01 scratch variables are optional test overrides. They do not configure a user application. For scoped local runs set CARGO_TARGET_DIR/TMPDIR/module-cache to own storage and Cargo jobs=1, use the installed offline toolchain. Record actual selection/binary/source/nonce, named ignored subprocess drivers and child cohort counts separately. The current runtime uses full durable snapshots per changed operation; release latency/RAM qualification is separate.

After material changes update the handoff, implementation ledger, P01 result and integration manifest with actual scope/results. Native restore UI/tab/caret/session lifecycle and complete DOC-01/P01 acceptance remain separate.

For public-path verification read `crates/latexide-core/src/services/recovery/cabi/CONTROLS.md`.
`driver.c` links against the public header and either a newly built `.a` or
`.dylib`; it requires macOS sandbox initialization/negative write probe before
the production factory and self-exits by 9s. `controls.py <own-scratch> <unique-attempt>`
expects `binaries/cabi-static`, `binaries/cabi-dynamic`, and a private
`receipts/input.json` with a fresh nonce. The task runner must pin actual
source/helper/library hashes and dynamic load commands before running it;
record argv, parent wait, exact own cleanup paths, fixture hashes and actual
case IDs. Build with own CARGO_TARGET_DIR/TMPDIR/module cache and jobs=1,
offline/locked. Never run a producer-report command blindly or reuse a stale
binary as current-source proof. A missing sandbox is blocked, not a reason to
drop confinement. Preserve all attempts separately.

Keep method counts separate: direct Rust storage/runtime, public native C ABI,
and authenticated Unix IPC. The 2026-11-02 local cohort has19 final controls,
41 own helpers and a fresh150-Rust-test regression with2 named ignored drivers;
32/5 direct child cohorts are separate units. This changes the earlier C ABI/IPC
unrun state only for those own synthetic sessions. No GUI/live app is implied.
v1 has no client-ACK/deduplication operation: after reply loss read/reconcile
text, revision and dirty state, never automatically replay the mutation. A
storage receipt is not proof of client receipt. Exact server write scheduling,
hostile same-UID behavior and Windows/Linux remain separate/unrun.


## Strict parent-cohort verification

The P01 child entrypoints are private fixture drivers, never standalone passing
tests. Keep their source pins and ignore attributes. `integration/registry/p01_ci.py`
separates normal test identities from the two exact parents, verifies the real
terminal parent outcomes and the 32 storage plus 5 runtime nested cases, and
rechecks source, executable, nonce, argv, fixture and raw transcript bindings.
Missing, unknown, stale or mismatched evidence fails the required verifier.
Do not combine nested helpers with the Rust framework count or turn historical
proofs into a fresh Cargo normal-summary claim.

Use the current main `integration/registry/verify.py` shared/integrated profiles
with an own fresh target and private fixtures, one Cargo job and existing offline
tools. The source-only inspection entrypoint does not execute or qualify tests.
The separate build-only actual schema qualifies its scoped image only; a fresh
combined workspace and required exact-HEAD CI remain separate gates. Preserve
current platform policy and published P00/P04/P19/P09 evidence when integrating
the older P01 source. Mac proof does not qualify Windows/Linux native storage,
GUI restore, hardware power loss, client ACK/exactly-once or production use.

The P18 profile contract in `tests/contracts/test_p18_ci.py` keeps its four
required execution cases, serial/include-ignored selection and compiler-PDF
dependency mandatory. Without P01 source it allows no exclusions. With the
source-pinned P01 provider it requires the four literal parent/helper roles,
the fresh cohort-proof and raw-build-log artifacts, and rejects foreign or
additional exclusions and missing/optional evidence. Its inert planning
fixtures exercise source completeness/hash binding on any Python host; they
are unit controls, not a Windows/Linux or Rust runtime result.

The combined workspace qualifier uses proof schema 3. When the inherited P18
TeX environment is present, preserve its Cargo run and validated provision;
the runner records the actual P18 environment passed to Cargo. Apply the
unchanged P18 case validator to its designated Cargo target: successful Cargo
stdout is captured, and method names may also occur in other test images.
give every normal-image replay an exclusively created private namespace below
the current cohort scratch. Keep phase roots and run labels separate from the
Cargo invocation and every other image. The namespace receipt binds run ID,
source, binary, driver nonce, phase, fixed argv and the inherited toolkit/proof.
Read-only rebuilding rederives those identities and checks actual raw phase
markers, PDF bytes, generation receipt, directory objects and file hashes;
missing/stale/foreign evidence or symlinks fail. PDF parsing/render equality
remains a separate required P18 gate. Never reuse or delete an earlier run to
make exclusive creation pass. The distinct build-only schema remains 2;
historical proofs are neither upgraded nor counted as current validation.
