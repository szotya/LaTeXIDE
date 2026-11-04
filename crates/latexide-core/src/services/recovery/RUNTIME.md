# Durable recovery in the single Rust Workspace

Implemented locally on macOS in the P01 runtime-integration slice (2026-11-01). This supplements [the private storage format](FORMAT.md); frozen contract-v1, legacy IPC, C ABI handles and agent authentication stay unchanged.

## Entry and authority

`Workspace::open_recoverable(project_root, storage_root)` loads a verified recovery state into that Workspace's existing document map and undo/redo stacks. `Workspace::open` retains its explicit in-memory behavior. On macOS the FFI factory `Runtime::open_project` selects `<project>/.latexide/recovery` before creating the same `Arc<Mutex<Runtime>>` used by native dispatch and authenticated MCP/IPC. Other native platforms retain the previous in-memory factory; explicitly requesting persistent storage there returns `recovery_unconfirmed_platform`.

There is one mutable Workspace. Encoded/candidate values exist temporarily for persistence and rollback; they are not another runtime. No restore operation writes a project document. The runtime reports actual recovery status (`enabled`, `restored`, `sequence`, `blocked`, `last_error`) in WorkspaceInfo. The native tab/caret/viewport and restore UI are separate unrun work.

## Private runtime formats, integrity and quotas

Each immutable `cohort-<32 lowercase hex>` directory contains an unchanged format-1 checkpoint and journal head plus `runtime-state.bin`. The latter binds document-relative ID, revision, logical text, original saved text, physical BOM/line-ending metadata, saved filesystem identity, last origin/transaction, both history stacks (byte patches/origins/transaction IDs), next transaction and optional pending-Save target.

`LXSTATE1` and selector `LXPTR001` use an 8-byte magic, little-endian u64 version=1, u64 payload length, payload, then 64 ASCII lowercase SHA-256 characters over header+payload. Payload variable fields use u64 byte lengths; collection sizes and all fields are bounded before allocation. The selector binds sequence, current/previous cohort name, complete metadata hash and encoded producer AckReceipt. Provider document/revision/content/dirty/encoding data must exactly match the decoded runtime state. Histories are validated against current text/scalar boundaries before becoming live undo/redo entries. Unsupported/corrupt selected state is an error; previous state is not an automatic fallback.

Existing limits remain: ten documents, 1 MiB physical/logical file ceiling, 256 history entries and 16 MiB aggregate patch bytes. The separate versioned runtime metadata budget is 64 MiB; the owned runtime root budget is 256 MiB. Current and previous cohorts are retained. After commit, only older recognized private runtime cohorts and unselected selector-pending files are removed. Unknown entries/symlinks/hardlinks or permissive modes fail closed and are not deleted. The earlier two delivery/evidence trees are outside this lifecycle.

The initial implementation synchronously snapshots full workspace/history state to private storage for each changed operation. Responses remain compact; no workspace snapshot is sent per keystroke. Release latency/RAM/throughput qualification is unrun; incremental storage optimization remains separate.

## Mutations and commitment

Open/edit/undo/redo validate through the existing core, persist the complete candidate and only then return success. Failed preparation/publication before selector rename restores text/revision/history. Invalid UTF-16, stale revisions and no-op changes do not advance a recovery commit. Checkpoint/journal, metadata and selector writes are flushed/synced; selector rename and directory sync form the local durable publication point.

A failure after selector rename is explicit `recovery_commit_uncertain`: the complete published candidate remains in memory and further mutations are blocked until reopen. This is not a successful request. A crash after publication can restore that complete state even if the IPC/C ABI reply was never delivered. Client reply delivery/exactly-once semantics are separate from the local commit point; the implementation does not claim proof of client receipt.

The producer's own r7/r8 receipt semantics remain unchanged: unselected prepared cohorts and raw journal tails are never selected from mere presence. Corrupt selected state does not silently load older data.

## Explicit Save and undo

Agent edits remain direct. A Save request first rechecks expected revision, canonical project path, saved raw bytes and saved filesystem identity. Recovery data grants no write permission. Restore preserves the old saved baseline, so changed or replaced disk files still produce `external_change` without overwrite.

Before project-file Save, the runtime commits a pending-Save intent with the original baseline and full history. The existing explicit file Save writes and renames the requested file; then the new baseline/identity and cleared intent are committed. Neither step erases undo or redo. A final recovery failure after a successful disk Save returns `save_completed_recovery_failed`, preserving the live undo history and the persisted intent.

On reopen, a pending intent is reconciled against the exact bounded regular file within the project, using a nofollow descriptor. Only bytes exactly matching the explicitly requested content establish the new baseline. Outside symlinks cannot mark the buffer clean. Old/missing/conflicting disk data preserves the original baseline and recovery text. Reconciliation writes recovery metadata only. Undo after Save (including after a restart) returns dirty text while disk stays unchanged until another explicit Save.

## Writer and platform boundary

Mac metadata uses 0700 directories, 0600 ordinary single-link files and the existing O_NOFOLLOW_ANY/ownership checks. A nonblocking kernel flock on the private writer descriptor prevents two cooperative persistent Workspaces from writing the same store. Descriptor close/process exit releases it; there is no PID scan or stale-lock deletion. This is not a hostile same-UID security guarantee or Windows ACL/lock qualification.

2026-11-01 local proof (historical): full Rust workspace 150 PASS/2 named ignored child drivers/0 failures/0 warnings; P01 recovery target 47 PASS/2 ignored (13 new core runtime tests); FFI Runtime factory/dispatch 2 PASS. The original storage matrix runs 32 own self-exits; the runtime matrix adds 5 cases (before/after edit publication, published Save intent, prepared post-Save final state, published undo after Save). Each child uses the exact current executable, a nonce and a 9-second self-exit watchdog; only its own parent polls/waits with a 10-second limit. Framework cases and child cohorts are separate counts.

Permission/sync/publication observer failures are injected. That proof did not cover public C ABI/IPC sessions. Own-process exit/reopen is not power-loss, kernel crash, F_FULLFSYNC/hardware-cache, release-performance, native GUI, live packaged app or Windows/Linux acceptance. Full DOC-01/P01 remains partial.

## Public C ABI / authenticated IPC local proof (2026-11-02)

The separate [C caller and controls](cabi/CONTROLS.md) use newly built static and
dynamic FFI artifacts through the unchanged public opaque handle. Agent Unix
IPC and native C calls use the same production runtime/session descriptor and
revision; a simultaneous same-revision edit race has exactly one winner.
Explicit Save, undo/redo, normal reopen and acknowledged IPC undo followed by
own process exit/reopen retain state/history. Torn selected journal and invalid
embedded storage receipts reject the C factory without falling back. The
non-observed r8 receipt rule remains a separate direct-Rust producer test.

Final cohort: 19 controls PASS, 41 sandboxed own helpers, 0 failure/error/skip;
fresh direct Rust workspace 150 PASS/2 named ignored with separate 32/5 child
matrices. The first 17-control run is retained separately, not added as new
independent cases. Product Rust/auth/session/wire/header/dependencies unchanged.
The C parent receives no result in the dropped-result control; IPC controls
disconnect after full request or consume only one reply byte. Reads on a new
connection and a new process reconcile state, with no automatic mutation replay.
These establish incomplete client observation, not the exact scheduler instant
of the server's write failure. v1 has no client ACK/deduplication contract, so
client-ACK/exactly-once qualification is explicitly blocked by the frozen scope.
Malformed/unknown ACK requests and missing/duplicate embedded storage receipts
are negative controls, not an implementation of client acknowledgments.

The helper requires a Mac sandbox and proves an own out-of-root write is denied;
it self-exits by 9s, the retained parent waits at most 10s. No controller kill,
foreign process/FD inspection or unsandboxed fallback. An incomplete frame
during public free exercised real StopJoin/read timeout (3.09s helper lifetime).
Only exact retained own session remnants are removed after own wait. This is
self-authored, same-host/toolchain, cooperative local evidence, not independent
security, GUI/live packaged app, Windows/Linux, hardware or production acceptance.


## Initial directory preparation: ancestor fix (2026-11-02)

P03-P01-RUNTIME-F01 found that rejection alone was insufficient: a `.latexide`
symlink caused `outside/recovery` to be created before the factory returned NULL.
The earlier 19 public-path controls did not cover that case. A separate own fresh
C ABI RED reproduced it; five own C ABI ancestor controls now pass after the fix,
along with the 19 public C ABI/IPC regressions and fresh 160-pass Rust workspace.
Historical source/review results remain unchanged and are not relabeled.

Recovery directory preparation now uses the owned `private_directory.rs` helper.
Before any creation, existing ancestors are opened read-only with kernel
O_NOFOLLOW_ANY/O_DIRECTORY and metadata/descriptor identity checks. Missing
components use mkdirat/openat relative to a held parent; new directories retain
0700/effective-owner checks, parent sync and final no-follow path/descriptor
rebind. Existing ordinary project ancestors may remain 0755; no chmod or repair.
Unsupported primitives fail closed without weaker-flag fallback. The runtime
ensure and producer cohort creation both use this boundary. Auth/wire/C ABI
and the one authoritative Workspace/Save/undo contract are unchanged.

Controlled stale-metadata parent-link, held-parent path replacement and leaf
insertion tests check symlink redirection. A directory already held by a process
can itself be relocated by another actor: this code is not a global OS-atomic
filesystem-confinement or hostile same-UID guarantee. Other-owner/unsupported
kernel injection, Windows/Linux, hardware/power-loss and live app remain unrun.
No new client ACK/exactly-once protocol is implied. Shared handoff/ledger/skill
updates are proposed in the own result/report for later integration, not written
concurrently by this slice.
