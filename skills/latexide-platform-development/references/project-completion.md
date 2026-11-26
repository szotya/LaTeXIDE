# Project macros, references and BibTeX completion

Read `<repo>/docs/project-completion-verification.md` and its linked design/review before extending the feature. Shared Rust source is `crates/latexide-core/src/{completion,tex_scan,project_index,workspace}.rs`; Mac consumers are WorkspaceStore, WorkspacePresentation and Editor/CompletionController. No new runtime dependency/language server is required.

## Authoritative project data

Index `.tex/.ltx/.sty/.cls/.bib` under the project root. Open Rust documents override their disk copies, including dirty agent edits and undo. Indexing must not open documents/tabs, consume the ten-document budget, save or mutate revisions/history. Deterministic precedence: active file → other open files sorted → remaining files sorted; last declaration within a file. This is lexical precedence, not TeX include order or arbitrary expansion.

Bounds:128files,8MiB total,1MiB/file,8192symbols,10000visited entries,20candidates. Skip symlinks/hidden/artifact directories and reject escaped canonical roots. Read source boundedly; invalid/unreadable/over-budget data yields partial coverage. Keep retained path strings shared rather than copying them for every symbol. Do not serialize full source on each query.

Disk discovery/metadata cache is1s; dirty open revisions bypass it immediately. Forced acceptance must re-read unopened content even with unchanged size/mtime. Already-open text follows the existing authoritative/external-change contract; do not silently overwrite it from disk. Document watcher/include-graph/LSP behavior is future work.

## Query and acceptance

Existing `get_completions {document_id,expected_revision,caret_utf16}` adds optional `refresh_project:false`. Response keeps absolute scalar-safe UTF-16 replacement range and insertion-relative selection. Items add `kind:command|macro|reference|citation`; optional `index_status {indexed_files,truncated,skipped_files}` reports coverage. Old consumers may ignore additive fields; updated native clients must use freshness semantics.

Before accepting, repeat the query with `refresh_project:true` and verify the selected range, label/kind, insertion and selection still match. A different file can change without changing the active revision. Stale Enter/Tab dismisses and falls through to ordinary editing. If insertion already equals source, consume acceptance and move the caret without adding an edit/history entry or literal tab. Remember the accepted document/revision/caret so pending callbacks do not immediately reopen the same popup; a changed context can query again. Otherwise insert once through the existing native/core adapter, preserving shared undo/IME/focus rules.

Enter/Return (including numeric-keypad Enter) must both accept and close the chooser in one action, keeping source focus. Escape closes without accepting. Verify actual native events and popup visibility/parent detachment after delayed refreshes, not just inserted text or a direct controller call. Include arrow-selected macros, already-complete/no-edit keys, citation lists, ordinary second Enter and shared undo. Mac characterization is `CompletionKeyboardTests`; latest evidence is `<repo>/docs/completion-enter-verification.md`. A currently passing characterization does not establish the cause of an intermittent user report or a new fix.

Decode/show index status even when there are zero items. Mac keeps a compact popup type badge/provenance and a status-bar partial hint; do not claim complete project coverage after a budget/read failure. This is actual index state, not a simulated progress signal.

## Lexical subset and failure recovery

Own macro families: new/renew/providecommand, DeclareRobustCommand, DeclareMathOperator, def/gdef/edef/xdef and New/Renew/Provide/DeclareDocumentCommand. Simple plain parameters and common `m/o/O/s` signatures get required braces; unsupported/delimited signatures insert bare names. Preserve existing stars/arguments. Control words are ASCII letters/`@`, max64bytes; keys max256bytes, Unicode alphanumeric plus `_:-./+!?@`.

Reference/citation command families and exclusions are enumerated in verification. Replace the whole current comma-separated key, including suffix after caret; preserve whitespace/other keys and omit keys already present elsewhere. A whitespace-only segment is a zero-length key at the caret. Repair a missing closing brace only on a final list segment; leave nonfinal unfinished lists intact. Do not insert an early `}` that ejects later keys.

Skip TeX comments/verb/verbatim-like/comment environments and BibTeX comment/string/preamble pseudoentries. Respect nested braced/quoted BibTeX fields. Context groups are limited to16KiB; unverified longer groups suppress suggestions. Short literal-name reads and aggregate signature lookahead budgets are essential: repeated `\begin{` once froze a near-limit query for seconds despite a per-group16KiB cap. A per-call bound multiplied by many commands is not an aggregate bound.

## Verification and ports

Exact macro/ref/cite navigation reuses this same index and precedence. See [definition-navigation.md](definition-navigation.md) for clicked-character ownership, normalized declaration ranges, forced read-only resolution and native target freshness. A completion prefix is not itself an exact definition hit.

Use real filesystem/workspace tests plus actual IPC and native controls. Keep malformed near-limit latency regression, ordinary cold/warm corpus timing, dirty cross-file undo, unchanged-mtime forced refresh, list/scalar/empty contexts, limits and nonmutation coverage. Core-only dev `opt-level=2` was chosen after measured cold cost; it retains symbols/assertions but can affect stepping. Do not call these pure-core timings release/end-to-end/RAM proof.

Windows/Linux must implement native popup/selection/fresh acceptance/field-local undo/status with their APIs. MSVC Rust check is not native execution. See `windows.md`/`linux.md`; preserve their unrun boundaries. Update the central handoff/ledger and this reference when changing a reusable parser/cache/wire/port contract; keep detailed chronological evidence in Markdown rather than here.

## Internal Runtime bibliography rename apply

Canonical bibliography workspace/completion/key-rename adapters feed an immutable,
source-bound preview into existing Workspace transactions. Internal Runtime code
uses the genuine Weak/try_lock owner, never copied display rows or a substitute
host. Consume the single-use apply token on every attempt. Keep final context,
analysis, retained revision/generation/ABA, canonical preview and scalar-safe
UTF16/expected-text checks under one owner guard before one atomic memory/history
group. Every target must already be open. Preserve duplicate/collision, stale,
missing/closed/foreign/busy, owner-gone and quota refusals. Disk observation is
sequential; hostile-race isolation and atomic multi-file Save are not established.
Public C ABI/Session/UI routing and explicit Save remain separate.

Controls belong to the cfg(test)=true FFI image; building normal Core/FFI does
not execute them. Bind normal Core extern paths, source/dep-info and actual image
before discovery/selection. A test-only default helper may create a fresh random
nonce under the caller's temp_dir without explicit P12_ROOT/P12_NONCE. Keep
exclusive fixture creation, raw BOM/CRLF checks and nonce/path/output bindings.

A locked dependency may issue a known negative feature probe. Classify only its
exact source/build-script/actual Cargo caller/argv/CWD/input/wait-reap binding;
real compilation/linking and all unknown children remain required zero-exit.
Read paths from actual artifact records, never another directory's derived hash.
Preserve qualifier ERROR/STOP and later correction as distinct evidence; do not
replay completed tests to repair a reader. The eight first main-default controls
stay separate from historical producer/reviewer cohorts. See the implementation
ledger for commands, results and remaining hosted/platform/release gates.

The production P04 filesystem analysis producer currently supports macOS only.
Positive Runtime bibliography-apply controls select Mac methods (the eight
original methods plus four review regressions); non-Mac CI selects explicit whole-refusal controls on the real owner
and production capture path. Require the exact unsupported-analysis error and
unchanged clean/dirty buffers, revision/history and physical BOM/CRLF bytes.
Keep those negative controls distinct from positive feature/native-platform
acceptance. Do not add a fake successful producer, ignored tests or a relaxed
required gate to make an unsupported host appear qualified. Bind every changed
public HEAD to its own hosted execution; a prior Mac PASS is historical there.

For citation apply, revalidate the original complete capture once and select the
winner from that same immutable bound canonical projection under the owner's
exclusive borrow. Preserve source/revision/digest/old-token checks; redundant
fresh projections do not add filesystem race isolation. Resolve sorted rename
UTF16 endpoints in one forward cursor per file and reuse validated Positions in
batch construction. Preflight the projected text with the retained physical
BOM/line-ending metadata and the normal shared Document encoder before issuing
a rename token; keep the apply-time size check.

Current P04 capture requires a text-only root, including unrelated assets. Keep
binary/non-text capture failure whole; explain this requirement and the original
producer refusal. The current error API does not expose an exact offending path.
Do not add a weaker post-failure reader or a successful filtered/model snapshot
for diagnostics. Distinguish ordinary tracked HEAD inventory (nonempty required)
from a staged deletion-only ACMR delta (no surviving content to scan), retaining
real-root and Git-error guards for both.

Keep the complete source-bound discovery partition: the current FFI image also
contains earlier recovery and P02 methods. A focused twelve-method bibliography
round must not classify its unselected methods as PASS, sum per-case filtered
counts, or replay completed discovery to repair an own counter. Admit dep-info
from actual compiler out-dir/crate-name/extra-filename fields, not a top-level
Cargo convenience artifact or another root. Before a strict catalog, materialize
source modes from the committed Git mode; do not relax a pin for a private
umask. Bind documentation-only result commits separately from the compiled
source, then require fresh hosted CI/review for the actual published HEAD.
See the implementation ledger for the current twelve-control evidence and
preserved metadata errors; this remains scoped same-host Runtime evidence.

For a multi-edit Document, resolve all original UTF16 line/column requests in a
forward pass and preserve per-input start/end/replacement error precedence.
Retain missing-line versus invalid-column/surrogate diagnostics, overlap and
encoded-size checks. Keep long-prefix same-line Runtime coverage and compare
batched coordinates with the unchanged individual resolver. A successful Cargo
partition is separate from the subsequent bounded normal-image qualification;
record its exact failure and dependentUNRUN rather than extending timeouts.

For an error assertion whose success type intentionally lacks Debug, extract
err().expect instead of adding an unrelated production Debug implementation.
Preserve a failed carrier compilation and the consumed attempt; qualify the
corrected carrier/source separately before using its execution as evidence.

Document::load binds a real file identity. A differential unit fixture must
create its exclusive owned file before loading its bytes. Retain fixture/raw
identity and label cfg(test) evidence separately from normal external Runtime
consumption. After a carrier-only fix, recompile that carrier; preserve the
previous normal component's exact source/image identity rather than relabelling
it as a fresh whole-candidate build. Full exact-HEAD hosted CI remains required.

A local carrier-only fixture correction can preserve the earlier normal product
component when its complete production source body/config and image binding are
byte-identical. Keep those local source cohorts explicit; do not rename old
normal Runtime executions as a fresh whole-candidate run. Require a fresh full
hosted gate on the final public HEAD, and keep every failed carrier and consumed
compile attempt. Publish only the reviewed final versionable tree over the
current public parent, leaving private failed intermediate history private.

Validate a normal child command/status/finite-time/log binding before its
namespace readers. Reject previously recorded capture-error fields and retain
the original phase/exit/timeout/capture cause; a missing derived namespace must
not mask that first failure. This strengthens failure handling without raising
timeouts, dropping nonce/source/image guards or accepting partial evidence.
Keep synthetic validator regression results separate from actual Runtime runs.

The fixed maintained Python entry runs on the actual own candidate HEAD. Bind
its complete source-derived selected method inventory; repair a result-reader
count from retained raw/source evidence without replaying already completed
tests. A positive Python validator cohort does not repair or replace a failed
actual Runtime/full-CI result. Keep original child status/capture errors and
require a new exact-HEAD hosted gate before publication acceptance or merge.
