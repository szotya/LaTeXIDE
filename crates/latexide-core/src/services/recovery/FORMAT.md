# P01 private checkpoint/journal format 1

This is an owned storage format, separate from frozen contract-v1 and legacy IPC v1. The producer stores read-only recovery candidates, never a second authoritative runtime or an automatic Save payload. One trusted writer is a precondition; no OS lock or multi-process ownership is implemented. Caller-selected `AckReceipt` is mandatory for cold recovery.

## Byte framing and bounds

All integers are little-endian. A frame is:

| Bytes | Field |
| --- | --- |
| 0..8 | `LXRECOV` followed by NUL (8-byte magic) |
| 8..12 | private format version u32 = 1 |
| 12 | kind u8: checkpoint=1, journal head=2, journal entry=3, caller receipt=4 |
| 13..17 | payload byte count u32 |
| 17..17+length | payload |
| next 64 | lowercase ASCII SHA-256 of the entire preceding frame header and payload |

Unknown versions/kinds, short headers/payloads/trailers, trailing fields, invalid booleans/UTF-8, overflow or over-budget lengths are explicit errors. Length bounds are checked before allocating a field. Strings/byte arrays use u32 byte count followed by bytes; no implicit base64 or JSON/wire change.

Every stored payload contains project ID, session ID, nonce and generation u64. Scope strings have a 128-byte bound, nonce is 64 lowercase hex characters. The nonce is trusted caller input (the test cohort generates it from OS randomness); this is not authentication or OS ownership. Reusing/forging a caller receipt is outside the source-authenticity guarantee.

Checkpoint payload: scope, generation, shared schema u32, checkpoint sequence u64, document count u32, then document ID, revision u64, dirty bool, encoding metadata `(physical_bom bool, crlf bool)`, logical UTF-8 byte array per document. Encoding metadata is a private recorded producer value, not a new field in the shared DTO or proof of native serialization. Logical leading U+FEFF and Unicode scalars remain content.

Journal head: scope, generation, entire checkpoint raw-byte SHA-256 string, checkpoint sequence u64. Entry: scope, generation, previous complete frame digest, sequence u64, document ID, revision u64, encoding metadata, logical UTF-8 byte array, logical-content SHA-256 string. The full frame and caller receipt bind sequence/revision/length/metadata/scope/content together; text SHA alone is insufficient. Replay requires sequence +1 globally, revision +1 per known document, exact encoding metadata and a valid prior-frame chain. Unknown documents/create/delete/save markers are unsupported.

Receipt: scope, generation, acknowledged sequence, complete checkpoint raw digest, acknowledged journal prefix byte count u64, raw prefix digest, final frame digest. A receipt frame has the same full framing integrity. Decoding proves byte integrity only; it cannot prove a caller received ACK. Recovery never chooses a higher sequence/generation from file presence.

Limits: 1 MiB per logical file and also per physical serialization implied by BOM/CRLF metadata; 10 retained recovery documents; 4096-byte document IDs; 1024 journal entries; 16 MiB active replay bytes. Producer reserves a total 16 MiB across **all** retained generations and pending files, at most 64 files, before writing. It does not delete earlier receipts/generations to make room. Quota rejection is not successful checkpointing. Retention/pruning policy is a later integration task.

## Write and caller ACK order

Create a new 0700 directory under an existing owned 0700 parent; sync that parent directory. Checkpoint/generation journal creation each use a new 0600 `.pending` file, complete `write_all` (with an observable half-write boundary), `flush`, `File::sync_all`, exclusive `renamex_np(RENAME_EXCL)`, then parent-directory `sync_all`. Reopen/verify the complete selected raw bytes and replay before returning `AckReceipt`. Existing generations/pending artifacts are retained on failure; a collision is an error, never overwrite.

Append: strict-validate the caller's current receipt and all sequence/revision/encoding/quota constraints; open the existing private journal, confirm its length, write the new frame in two bounded segments, flush, sync the file, sync the store directory, read back/compare the complete prefix, validate replay, then return a new receipt. Any write/flush/sync/rename/directory-sync error returns a phase-tagged error, without ACK. Hooks before/after phases expose deterministic boundaries; a successful hook does not replace the real IO call. Injected hook errors are labelled synthetic in tests.

A caller observing a receipt after method return is a separate event. The fixture persists that observed receipt with its own file+directory sync. An exit after write or sync but before method return cannot promote r8 when the retained caller receipt is r7. An exit after caller observation of r8 may recover r8. The producer has no automatic latest-ACK file or transport/native acknowledgement hook.

## Strict recovery and explicit valid prefix

`strict` requires exact raw journal length, checkpoint digest, journal-prefix digest, framing, scope/generation chain and replay agreement. Any additional/torn/unacknowledged tail yields an error mapping to v1 `Corrupt`; no successful partial recovery is returned.

`recover_explicit_prefix` is a separate opt-in operation. It retains the original strict error, v1 error, selected receipt digest, rejected tail length/digest and separately verified recovered prefix. It never truncates or overwrites storage. If acknowledged bytes/checkpoint are damaged, prefix recovery also fails. Rotation checkpoints exactly the caller-selected state into a new generation while keeping prior generations so the earlier r7 receipt remains usable across every transition.

## Platform and evidence limits

Storage currently confirms macOS only; other hosts return `UnconfirmedPlatform`. Mac storage checks owned 0700 directories, 0600 ordinary files, euid, single link, descriptor device/inode identity and `O_NOFOLLOW_ANY` across path components. Leaf traversal, symlink, hardlink and permissive-mode fixtures are rejected. The verified SDK flag is used alone: combining `O_NOFOLLOW_ANY` and `O_NOFOLLOW` produced EINVAL in a preserved initial attempt. Same-UID hostile mutation and concurrent writers are not an OS-lock guarantee.

The process-boundary matrix uses only a newly spawned copy of the current test executable, an exact ignored driver selection, a cohort nonce, own files and `_exit`. Each child has a 9-second self-exit watchdog; the parent only waits/polls its own child for at most 10 seconds. It does not scan or kill other processes/FDs. The full matrix has 32 points across append/checkpoint before/half/full write, flush, sync, rename, directory sync, before ACK and after caller observation.

`sync_all` success and this own-process exit/reopen evidence are not power-loss, kernel-crash, F_FULLFSYNC/hardware-cache, Windows or Linux durability qualification. Real disk-full/permissions/sync/rename faults are **injected**, not filesystem-wide experiments. Native runtime/Save hook, caller receipt lifecycle, saved disk baseline, undo/history, OS ownership/locking, cleanup and native restore are unimplemented/unrun. The one runtime/direct edit/explicit Save/undo contract is unchanged. Integration needs are in `integration/requests/P01.json`.

## Subsequent runtime binding

The private producer above stays format 1. The later single-Workspace integration supplies separately versioned baseline/history/selector metadata; its current implementation, Save/reply/retention boundaries and evidence are documented in [RUNTIME.md](RUNTIME.md). The earlier producer-only unrun statements above describe that earlier slice.
