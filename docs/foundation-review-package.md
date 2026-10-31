# Whole-foundation review package

No Git repository exists; review the current source tree as the entire new change, not a fabricated diff. Read-only review. No edits, installs, publication or subagents. Return findings to the parent.

Accepted spec: `docs/superpowers/specs/2026-10-30-native-latex-ide-design.md`.
Plan: `docs/superpowers/plans/2026-10-30-native-ide-foundation.md`.
Rulings and actual evidence: `docs/implementation-progress.md`, `docs/verification.md`, `docs/DEVELOPER_HANDOFF.md`.

User accepted shared Rust + two native frontends, M-series Mac only, Windows11 x64 Intel/AMD, official open-source local compiler provenance, local-agent MCP direct edits with undo after save. Latest addition: no Windows runner exists; maintain all substantive Mac work in Markdown and a reusable project skill for Windows/future Linux developers. Linux UI not selected.

Implemented: five Rust crates (documents/protocol/C ABI+IPC/build/official rmcp bridge), actual verified local TeX Live, Swift/AppKit/SwiftUI/PDFKit `.app`, Windows C++/WinRT WinUI source/project/build boundary, compiler acquisition scripts, tests/docs/skill.

Actual checks: 13 core +6 supervisor; four IPC; three compiler-runtime; one real MCP stdio; five native Swift tests. Mac app launched; actual remote edit+save visibly appeared, native menu undo succeeded after save, re-save+real TeX Live build rendered PDFKit. First debug RSS170MiB. All five Rust crates type-checked for Windows; native Windows link/runtime UNRUN. Portable Windows TextAdapter functions exercised on Mac only. See logs, not promises.

Known unfinished release scope: Windows execution, broad corpus/hardware IME/accessibility/stress/release-memory; syntax highlighting/search, full external file watcher/disk build manifest, crash recovery, notarized packaging. Current Mac GUI metadata polling250ms rather than callbacks; direct per-keystroke UTF16 ranges, full document only on external revision changes. A GUI keypress through computer use did not visibly invoke undo; the actual native menu worked and a direct AppKit key-routing test passed. Do not infer hardware keyboard acceptance from that test.

## Review Focus (verbatim from plan)

- UTF-16 edits around emoji/combining characters and CRLF must not split or silently alter text; Task 2 owns these tests.
- An agent edit followed by user input, save and undo must retain ordering and not erase unrelated user changes; Tasks 2/5 own these tests.
- External file changes and failed writes must not be reported as successful saves; Task 2 owns these tests.
- An MCP client with a wrong session token or project traversal must not access another workspace; Task 3 owns these tests.
- A failing/cancelled compiler must not leave orphan processes or relabel an old PDF as fresh; Task 4 owns these tests.

Judge by actual user effect, not whether the vision spec enumerates an input. Distinguish source bugs/material failures from acknowledged unrun platform checks and genuinely later product scope. Check FFI ownership/panics/lifetime, save/undo, IPC host shutdown and isolation, frontend composition/revisions, process cleanup, compiler env, Windows source plausibility, and documentation accuracy. List every behavior considered and set aside under Declined to judge, with reason. Return Critical/Important/Minor with exact file:line, reproduction/effect and suggested fix, plus a clear assessment. Do not modify source or dispatch another reviewer.
