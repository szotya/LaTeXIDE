# Project completion — single integrated review and fixes

2026-10-31. One independent fresh-context read-only review covered the shared project index, lexical/context parsing, additive IPC, native acceptance/presentation and tests. No Git baseline exists; review inspected the named current files against the spec and real evidence. No re-review was performed.

The reviewer requested changes for P1/P2 plus a P3 whitespace case. The author accepted all three and regraded the whitespace case **Important**: a user regularly types comma + space before selecting the next citation. All three were fixed in one reproducing-test pass. An additional exact-key Tab defect found during that pass was reproduced and fixed too.

| Finding and effect | Reproduction before change | Final behavior |
| --- | --- | --- |
| P1 / Important: repeated malformed group lookahead blocked synchronous native queries. | Reviewer:700008-byte repeated `\begin{` input took4.686s. Own900KB regression measured5.398s; malformed newcommand also1.370s. `review-red.log`. | Literal environment detection uses64-byte lookahead; macro names/arity use short lexical reads; document signatures share an aggregate lookahead budget, with bounded label/def parsing. Same five near-limit own inputs now3.8–43.3ms in the recorded run. A generous1s regression guard catches the multi-second stall; this is not a release latency qualification. |
| P2 / Important: completing an early key in `\cite{al<caret>, beta` inserted `alpha}`, ejecting beta from the argument. | Core and actual AppKit Tab tests produced `\cite{alpha}, beta`. `review-red.log`, `review-native-red.log`. | Replace only the key. An unfinished **nonfinal** list segment gets no closing-brace repair; later keys remain inside the unfinished argument. Final missing closers may still be added. Citation and reference tests cover this. |
| P3 → Important: whitespace-only list segments offered no candidates. | `\cite{alpha, <caret>}` returned none; native Tab inserted a literal tab. Same RED logs. | Treat a whitespace-only segment as a zero-length key at the caret; preserve surrounding whitespace, exclude other already-used keys. Core and native comma-space cases pass. |
| Additional Important: accepting an already complete key fell through to ordinary Tab and modified the key. | Actual native test produced `\ref{sec:overview\t}`, revision2/history instead of unchanged revision1. `exact-key-red.log`. | After fresh validation, an identical insertion closes the popup/moves the caret and consumes the key without editing source or adding history. |

A final native acceptance regression also reproduced immediate reopening of the same key popup after a pending refresh (`accepted-popup-red.log`). Acceptance now remembers its final document/revision/caret until context changes; the real native refresh test passes. This stays in the same fix pass, without another review.

All evidence paths above are under `evidence/project-completion-2026-10-31/`. Final full suites:56Rust passed; native57executed/53passed/4opt-in skipped,0failures; separate project render1PASS; IPC9, stdio MCP1, actual compiler3 and all-five-crate MSVC x64 check PASS. Exact commands, logs, live observations and package closure: [verification](project-completion-verification.md).

## Author rulings on every declined scope

- **Final package/signature/live workflow:** parent-owned checks were pending during review; completed separately with a newly staged arm64 app, actual typing/Tab/undo/redo, dirty cross-file agent label, restored sources and successful PDF/PDFKit. Controlled renders and live capture limitations remain distinct.
- **Windows/Linux runtime:** retain unrun status. Shared Rust type-check and reusable native contracts do not establish target UI, keyboard or process behavior; those need actual target machines.
- **Release performance/RAM/accessibility/template corpus:** retain unqualified status. Small bounded core timings, allocation improvements and native tests do not replace end-to-end release measurements or broad compatibility testing.
- **Documentation closure:** parent owns current handoff, ledger, README, product capability matrix and project skill. This work updates those; historical phase logs stay historical.
- **Full TeX/language-server semantics:** retain the explicit lexical subset. No execution, arbitrary expansion/catcodes, include-order inference, external installed package catalog or citation resolution is promised. Cost: dynamic declarations and unsupported signatures may get a bare command or no candidate.
- **Historical regression attribution:** no Git history was invented. Source/evidence and retained rollback app bundles are the local record; no claim is made that every behavior outside this increment was introduced here.

Additional implementation rulings: keep the existing architecture and no-commit/local-evidence workflow under the user's continuation request; use an optimized **core-only dev profile** after measured cold-query cost (debug symbols/assertions remain, but stepping optimized core code may be less direct); keep deterministic lexical precedence active document → other open documents sorted → remaining files sorted, rather than infer compiler scope. These are implementation choices, not extra choices attributed to the user. No deferred minor finding remains from this review.
