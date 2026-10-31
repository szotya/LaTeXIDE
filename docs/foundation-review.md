# Independent whole-foundation review — 2026-10-30

One fresh-context, read-only senior review was performed against the accepted spec, implementation plan and `foundation-review-package.md`. No reviewer mutations, installs, GUI interactions or new runtime tests were performed. Verdict: request changes before calling the foundation complete. No Critical findings; six Important and three Minor findings. The original verdict precedes the author's fixes below; no second review is claimed.

## Important findings and author disposition

1. **Mac unsaved project switch:** opening B/reopening A replaced the runtime without checking dirty documents. Fixed with protected programmatic open and a shared native save/discard/cancel transition for UI open/quit. Regression `testProjectSwitchPreservesUnsavedWorkspace` failed first, then passed.
2. **Rejected IME commit lock:** invalid `end_composition` left core and GUI composing, blocking future work. End now consumes the composition on validation failure; both adapters restore acknowledged contents on rejection. Actual IPC and AppKit near-limit regressions failed first, then passed. Windows adapter source is updated but not executed.
3. **Dotted source PDF name:** extracting `file_stem` then replacing its extension lost a suffix such as `.v1`. One helper now replaces the original filename's final extension once. Controlled-child regression failed first, then passed.
4. **Real XeLaTeX diagnostics:** the parser only recognized synthetic lowercase prefixes. Added actual `file:line:` errors, warning markers and `!` errors, preserving Windows drive-colon paths. A captured-style fontspec failure regression failed first, then passed.
5. **Windows compiler prefix mismatch:** profile used `.local/texlive-windows-x64`, build staging expected `.local/texlive-windows`. Standardized scripts/docs on the profile's x64 prefix. Source consistency checked; real Windows installation/build remains unrun.
6. **Hardcoded Mac installer checkout:** the installer passed a profile containing the original developer root. The checked-in file is now a template; an operation-local resolved profile and its effective paths are recorded. A different checkout with spaces failed for the missing renderer first, then passed. This is a profile-generation regression, not a full second installation at another path.

Validation after the fix pass: Rust 21/21 contract tests (13 core, 8 supervisor); Swift 7/7; IPC 5/5; actual MCP 1/1; compiler-runtime 3/3; profile renderer 1/1. Evidence includes `review-*-red.log` and current green logs named in `verification.md`. No re-review was dispatched.

## Minor findings

- **Deferred:** a separate agent save without a new text revision can leave the Mac selected-file modified badge stale. The core's disk/save/undo state is correct; this is a compact-metadata refresh issue to fix next. Full text reload is not necessary for it.
- README and central handoff had stale Mac/MCP/RSS/Windows-check summaries. These were reconciled as the ongoing documentation maintenance explicitly requested by the user; they are not additional behavioral changes.

## Behaviors the reviewer set aside and author rulings

- Windows restore/MSBuild/link, actual RichEdit/paragraph/IME, PDF, ACL and Job Object cleanup: remain required, unrun acceptance checks. No machine exists. Cost: Windows source may still require fixes, especially immediate children before Job Object assignment; no Windows-ready release claim.
- Real keyboard/broad IME and accessibility: retain narrower adapter/menu evidence; require target-device testing. Cost: hardware-specific behavior is not qualified.
- Search/highlighting, full disk watcher/build manifest, recovery and notarization: remain later implementation/release work. Cost: this foundation is not a complete student-release IDE.
- PDF freshness after arbitrary external disk writes: current staleness observes open in-memory revisions only. Keep this release limitation explicit; cost: a file changed outside IDE/MCP is not automatically reflected in the preview's freshness.
- Malware freedom, full native/dependency audit, frozen repository and broad templates: remain unverified. Cost: provenance is not a malware certificate or complete compatibility qualification.
- TeX filesystem sandbox: shell-escape/network restrictions are not filesystem isolation. Cost: arbitrary untrusted projects need a separately designed OS policy.
- Release memory/performance: the debug 170 MiB snapshot is not release acceptance. Cost: performance targets remain unproved.
- Adversarial filesystem races, same-user IPC flooding, process-group escape and FFI contract violations: targeted hardening requires a defined threat model beyond the present ordinary-workflow evidence. Cost: no blanket adversarial robustness claim.
- Last-window close cancellation and Windows async PDF completion during project replacement: require lifecycle runtime tests, not a source-only assertion. Cost: lifecycle edge cases remain unqualified.

All limitations are retained in the handoff. A separate reviewer is not claimed to have approved the final edited source; the author's reproducing tests and green suites verify this fix pass.
