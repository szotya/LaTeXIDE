# P01 C ABI / authenticated Unix IPC controls

Pre-registered for P01-CABI-IPC-RECOVERY-ENDTOEND-20261102-7c91a6e2.
Run only on macOS, offline, with newly linked static/dynamic FFI artifacts and
own private projects/nonces. No user application. No protocol/auth changes.

The C helper uses the public header/opaque handle, native C dispatch, matching
Rust string free, a self-only 9-second alarm and a required sandbox with a
negative write probe. Each parent retains its own Popen handle and waits at
most ten seconds; no kill, PID/FD scan or unsandboxed fallback. A missing sandbox
is blocked. Only synthetic descriptor/token files are read; no token is logged.

Controls (separate units; assertions/subcases are not added to case counts):

1. Dynamic and static C ABI: edit, explicit Save, undo/redo; reopen acknowledged
   text/revision/history, undo still leaves saved disk bytes untouched.
2. Authenticated IPC and native C ABI see one descriptor, revision and recovery
   sequence; race two edits at the same expected revision: exactly one succeeds,
   the other conflicts. Origin is agent/user as appropriate.
3. BOM/CRLF, logical FEFF, non-BMP UTF-16 and invalid scalar boundary. IME keeps
   native composition ownership and blocks agent mutations until finalization.
4. External file replacement with identical bytes and external changed bytes;
   Save must reject without overwrite. Stale revision must reject after reopen.
5. Auth/version/unknown-ACK rejection; missing token, malformed/duplicate JSON
   fields and incomplete request delivery do not commit a mutation.
6. Disconnect after full request without observing any reply, reconcile by read
   on new connection, then reopen. No mutation replay. Partial response consumed
   and discarded is uncertainty; observe text/revision through a new connection.
7. C ABI result deliberately dropped after real dispatch, process exits itself;
   reopened state is locally committed, client reply delivery remains unproved.
8. Partial/torn selected journal, truncated selector, malformed/missing/duplicate
   embedded receipt: factory must fail closed, retain corrupt bytes and never
   silently restore the previous cohort. Previous producer r7/non-observed-r8
   receipt behavior is tested separately through direct Rust, never called IPC.
9. Private modes, cooperative same-store owner lock, private session descriptor,
   normal StopJoin teardown and release/reopen; process-exit session remnants
   cleaned only by the owning parent using retained exact paths, after wait.
   Follow-up subcontrols explicitly keep a frame incomplete during public
   runtime_free (real IPC read deadline must bound StopJoin), and self-exit
   after an observed IPC undo reply following explicit Save, then restore the
   same revision/dirty text and redo in a fresh process.

There is no client ACK/deduplication operation in v1. Unknown ACK is a protocol
negative control, not an ACK implementation. A complete mutation request whose
reply was lost may be committed. Recovery cannot prove that a client observed
the reply. True client-ACK/exactly-once qualification is blocked by the frozen
protocol scope; neither storage receipts nor native Rust tests substitute for it.

These are same-host, same-toolchain, self-authored white-box fixtures; not an
independent review, native GUI, hostile same-UID, Windows/Linux, hardware-cache,
kernel/power-loss or production qualification. Existing bounded IPC deadlines
are exercised without changing them. Cases must report actual methods/counts,
child exit/wait/cleanup, source/helper/library/fixture/nonce/log identity.
