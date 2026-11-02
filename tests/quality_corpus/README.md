# P19: bounded fault harness, first slice

`run.py` implements the Python adapter for frozen v1 `QualityCorpusPort` /
`FaultExpectation`. Tests import this exact file by path. No shared registry,
Cargo manifest, Rust trait implementation, runtime hook or new MCP tool is
installed. The shared `p19.json` request shape is consumed; its response example
is never an oracle. The strict local decoder is not the complete shared
`CallContext` validator or the authenticated production IPC decoder.

Set `P19_SCRATCH` to an existing, private, external scratch directory and create
its `tmp` child. Run from the repository root using an existing Python runtime:

```sh
PYTHONDONTWRITEBYTECODE=1 TMPDIR="$P19_SCRATCH/tmp" python3 tests/quality_corpus/test_harness.py
PYTHONDONTWRITEBYTECODE=1 TMPDIR="$P19_SCRATCH/tmp" python3 tests/quality_corpus/run.py --report "$P19_SCRATCH/synthetic-matrix.json"
```

Optional `P19_TEST_REPORT` records the actual unittest method counts, selected
IDs, subtest outcomes, failures, errors, skips and filtered count. Zero discovered
methods exit nonzero. A passing negative-control test means the harness rejected
a faulty producer; it does not turn that producer result into PASS.

The only executable accepted by `run_fault_case` is the owned
`synthetic_helper.py`, launched through the running interpreter with `-I`, a
private working directory, no shell, and a small environment. It self-exits 86
at the checkpoint; recovery is a second private helper invocation. Timeout kills
and reaps only that owned child. This is ordinary process failure simulation,
not kernel power loss, physical disk full, directory-fsync qualification, a
production multi-file atomic rename, or actual P01 recovery. No live app,
daemon, user project, IPC listener or TeX/compiler is involved.

For every point, previously acknowledged revision `ack` must survive and no
partial edit is permitted. `before_journal_sync` refers to the unacknowledged
next record; `ack` is already durable. `after_rename` may leave the complete new
disk variant. The other points retain the old disk variant. Recovery does not
perform an implicit save. These are the explicit own-model oracles for this
slice, not an extension of a production service contract.

Every private case stores a request, nonce, stable case/input digest, source
map, helper/interpreter digests, pre-fault and post-recovery sequence/point/ack
events, raw project/journal/frame/recovery bytes, stdout/stderr, process exit,
timeout, skip and result. Actual bytes are checked independently of the
producer's `partial_edit_allowed=false` claim. A missing executable, zero result,
skip, malformed event, wrong binding, partial disk/recovered state, timeout or
unexpected exit cannot pass. `recheck_fault_result` checks the complete artifact
set and saved verdict; it detects omitted or changed records. Digests establish
binding/integrity, not producer authenticity. They do not qualify an arbitrary
provider.

`run_properties` uses seed 1901, at most 16 cases and at most 3000 ms; the default
16 own-model rejections cover schema, stale revision, scalar-safe UTF-16 and the
unchanged 1 MiB logical-file quota. The separate framed JSON control preserves
the 1 MiB encoded-frame limit. No seed/case escalation or third-party fuzz
dependency exists. `run_model_session` is a test-only, bounded edit/save/undo
model; its trace is not a long product soak or a second runtime authority.
`negative_partial_edit_model` is intentionally wrong and only exercises the
deterministic, budgeted reproducer reducer. Failed fault cases retain the exact
replayable case separately from fresh per-run nonces.

The corpus has `singlefile` and `multifile-hu` profiles, five own input files
including TeX/BibTeX and an SVG reference. Recipes, expected results, missing
inputs and platform rows are explicit in `corpus/manifest.json`. The readable
source files use LF. The multifile profile's `raw_bytes` are lossless JSON number
arrays that materialize BOM+CRLF without depending on checkout line conversion;
the chapter keeps a logical leading U+FEFF behind its physical BOM. Stored
source, materialized raw bytes and logical UTF-8 have separate identities.
Validation never rewrites fixtures or baselines. SVG conversion policy, compiler
versions, fonts/emoji coverage, PDFs, reference resolution and all native target
platform recipes remain unrun/missing.

P00 must integrate a real P01 checkpoint/recovery provider and P03 framed-runtime
provider in a later authorized slice before any product acceptance. See
`integration/requests/P19.json` and `docs/development/results/P19.md`. No QA-01,
QA-02, QA-05 or QA-09 requirement is closed in full by this self-check.
