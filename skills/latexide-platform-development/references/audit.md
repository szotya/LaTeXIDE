# Capability audits and implementation backlogs

Use this reference when assessing IDE completeness, planning parity work or qualifying a release. Start with the current `<repo>/docs/DEVELOPER_HANDOFF.md` and active user task. The dated assessment at `<repo>/docs/IDE_AUDIT_2026-10-31.md` and its `<repo>/docs/ide-audit-2026-10-31/` data are historical baseline inputs, not an instruction to implement every proposed feature.

## Evidence and scope

Record the source identity, test inputs and packaged artifact separately. If there is no Git repository, a content-hash manifest is an appropriate local baseline; an audit does not require initializing Git. Refresh source and maintained context when another increment changes the workspace during the assessment.

Keep these fields separate for each capability:

- Current implementation boundary and its code/service owner.
- Evidence type: source, type-check, linked build, controlled tests, real process, live UI, rendered image or release qualification.
- Proposed change and its implementation state. A backlog entry remains planned until actually delivered.
- Acceptance scenario, prerequisites, platform implications and remaining uncertainty.

Intentional prototype limits can be legitimate development choices and still be product gaps. A green regression suite does not remove untested scope. Verify behavior at subsystem boundaries with disposable projects, especially background reads versus retained editor buffers, external disk changes, build input identity, artifact validity and IPC cancellation/deadlines. Do not use a user's open dirty document as an audit fixture.

## Comparisons and priorities

Use current primary vendor/upstream sources. Distinguish built-in functions from plugins, editions and separately managed services. Check product lifecycle: retired/merged JetBrains products and extension compatibility cannot be assumed current from an old product list. Compare LaTeX workflows with the relevant LaTeX plugins as well as the editor platform.

Separate the LaTeX/research workspace from full polyglot IDE verticals. A language server, debugger adapter or new panel does not by itself deliver JVM, .NET, embedded, notebook or database product parity. Keep those requirements visible with their larger scope.

Prioritize correctness and data preservation before new views: document/recovery lifecycle, workspace transactions, common project manifest, source/artifact identity, async service ownership and native focus. Preserve one Rust document authority, revision/UTF-16 validation, direct agent edits and shared undo after explicit save. Optional diff review is an after-the-edit capability, not a new per-edit approval requirement.

## Reproduction and maintenance

The dated audit's generator and probes live under `<repo>/evidence/ide-audit-2026-10-31/`; inspect them before reuse. The probes use a Mac debug FFI library, private Unix IPC and disposable `/private/tmp` fixtures. They reproduce known boundaries; they are not a portable Windows/Linux acceptance runner. A future fix may intentionally change their expected observations.

For a new assessment, use a new evidence root. Save exact commands/results, validate the structured data, check prerequisite cycles and relative links, and preserve prior evidence. Run product checks appropriate to actual changes rather than repeating unaffected suites. A sandbox IPC `EPERM` must be identified and, if authorized, rerun with the needed local permission; do not remove authentication to make a test pass.

Publish a platform result only with that platform's evidence. Mac native tests and portable C++ helper checks do not establish WinUI/MSBuild, Windows named-pipe/Job Object or Linux UI runtime behavior. Whole-app latency/RSS needs an identified release build, corpus and process tree; a core microbenchmark is a narrower result.

Update the current handoff and chronological ledger after a material audit or qualification change. Keep source-backed conclusions, hypotheses and unrun checks distinguishable. A dated backlog is planning input; the active task and accepted product direction decide what is implemented next.
