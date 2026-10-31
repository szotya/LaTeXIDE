# Definition navigation — one integrated review

2026-10-31. One fresh read-only reviewer evaluated the [design](superpowers/specs/2026-10-31-definition-navigation-design.md), [plan](superpowers/plans/2026-10-31-definition-navigation.md), implementation and test evidence. No Git repository/range exists; the review covered the explicitly listed shared core/protocol/native files and their tests. The reviewer did not edit the checkout or operate/restart the user's app. No second review is requested.

Initial verdict: **with fixes**, 0 Critical / 2 Important / 1 Minor. Strengths: shared Rust authority/index, normalized coordinates, forced disk refresh, target revision/range/text validation and no navigation edit transactions. Author dispositions below grade effects on an in-progress research workspace.

| Finding | Disposition and reproduction |
| --- | --- |
| Important/P2: last macro character before a comment/verbatim boundary lost its definition hit. | Accepted. Completion was queried at the clicked character's right boundary, allowing the following ignored token to own it. New every-character macro/comment/verb/verbatim test failed at the final `n`; definition context now uses the clicked character, advancing only a leading backslash into the control-word caret context. Completion caret semantics stay unchanged. A native final-letter Cmd-click case supplements the core test. |
| Important/P2: a completed navigation replayed after all tabs were hidden and the editor recreated. | Accepted. The old request remained in the store, but consumption lived in the disposable coordinator. Native reproduction failed with `{2812,5}` and scroll3159 instead of the user's caret0/scroll0. `WorkspaceStore.takeNavigation` now owns nonpublished single-consumption state across coordinator recreation; native revision guards still apply. |
| Minor/P3: duplicate BibTeX entries bypassed per-file last-declaration precedence. | Author regraded Important for consistency while editing/merging bibliographies: the selected definition and alternatives count contradicted the shared winner contract. New duplicate-key test failed with line1 instead of line2. Last-per-file deduplication now wraps both TeX and BibTeX parsing, including partial parses. This does not validate or repair duplicate-key bibliographies; it makes indexed navigation/completion deterministic. |

All three were reproduced before their fix in `evidence/definition-navigation-2026-10-31/review-core-red.log` and `review-native-red.log`. The recreation regression then passed in `review-native-green.log`; full affected Rust/native suites and final packaging are recorded in the [verification report](definition-navigation-verification.md). This is one author fix pass, without re-review. No deferred Minor finding remains.

## Behaviors considered and deliberately bounded

- Expanded include/scope/catcode semantics, additional command families and comment-elided argument keys retain the existing bounded literal-key grammar. These are documented coverage limits; guessing TeX execution would make this increment less predictable. Cost: unresolved/different runtime scope can need manual navigation.
- After a target becomes a retained authoritative buffer, external disk changes retain the existing save-conflict boundary. General VFS refresh is separate; silently reloading would risk dirty text. Cost: external-only changes need the later watcher/reload workflow.
- A failed navigation that already opened a target can consume an editor slot. The accepted ten-buffer lifecycle is preserved; eviction is separate. Cost: document-limit feedback may require a later lifecycle improvement.
- Bounds and existing malformed-input regression pass; this review adds no release latency/RAM result at maximum budgets. Cost: performance qualification remains a separate measurement.
- Native control events are tested; physical/window-server dispatch, Fn/Caps Lock qualification, accessibility and Windows/Linux native behavior remain unrun. Cost: no broad hardware/target-platform acceptance claim.
- Documentation was being closed during the review; final handoff/skill/link/bundle evidence belongs to the verification closure, not the reviewer's initial result.
