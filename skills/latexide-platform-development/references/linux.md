# Future Linux port reference

Linux is a later platform direction, not an implemented or validated target. The user requested this reference so a later developer can continue from Mac work without repeating discoveries.

The private repository's Ubuntu CI now executes 69 shared Rust tests successfully. This narrower core/compiler-supervisor result is not Linux desktop, local IPC/runtime-directory, TeX installation or native UI qualification. See `docs/development/REPOSITORY.md` and the current ledger for exact CI source identity; do not infer the remaining port below is complete.

Start with the current handoff and the accepted Rust/native separation. A Linux task must select a native frontend technology and PDF/text adapters appropriate to the requested Linux environments; GTK/libadwaita or Qt have not been selected by the user. Do not imply that SwiftUI or WinUI is a Linux frontend.

## Concrete existing portability gaps

| Area | Current implementation | Linux work |
| --- | --- | --- |
| Runtime directory | `crates/latexide-ffi/src/session.rs` uses `/private/tmp/latexide-{uid}` for all `cfg(unix)` targets. | Separate macOS from Linux; use an appropriate XDG runtime directory or private fallback, with owner/mode/symlink checks and Unix socket length limits. |
| Developer bootstrap | `scripts/acquire_tools.py` installs the pinned arm64 Mac Rust payload and downloads the Windows compiler candidate. | Select actual Linux architecture/toolchain and official verified artifacts. Do not execute Mac payloads. |
| Compiler | TeX Live 2026 Mac runtime and Windows profile are present. | Select Linux TeX Live binary platform, retain common package roots/verification, and run own Unicode/multifile/offline corpus. |
| Native GUI | AppKit/SwiftUI and WinUI directions only. | Choose and implement editor, window, clipboard, keyboard/IME, accessibility and PDF adapters. |
| Integration tests | C harness linker flags and Python AF_UNIX tests were developed on Mac. | Adapt linker/platform paths and run on Linux; same transport name does not prove runtime-directory correctness. |
| Processes | Unix process groups/SIGKILL are used for compiler descendants. | Exercise cancellation, timeout, inherited output pipes and application shutdown on Linux. |

## Contracts that remain shared

UTF-16 edit positions are a wire contract even when the Linux widget uses UTF-8 byte offsets or Unicode scalar indexes. Convert deliberately, including emoji and combining characters. Keep one Rust state, revision checks, direct undoable agent edits, explicit save and existing disk-conflict protections. Route the widget's undo commands to the shared history.

Keep runtime descriptors private and nonce-protected; use the existing protocol/frame/chunk limits. Preserve compiler environment isolation and fixed source/output generation semantics. Never replace a working authentication or revision check merely to make a port test pass.

Bring a real Linux release build, GUI run, rendered editor/PDF verification and memory measurements into the handoff. Until those exist, use `planned`, `partial` or `unrun`. Keep Mac-only workarounds scoped: the TeX Live whitespace-TMPDIR issue is a verified installer behavior, whereas `/private/tmp` is an implementation assumption to replace, not a Linux standard.

## Reuse the developer UI contracts

Also read [intelligence.md](intelligence.md): completion reply schema and logical-key/focus/state ownership are shared contracts; the native popup and initial Codex process client are currently Mac implementations. Do not copy Darwin pipe flags or AppKit lifetime assumptions into Linux. Add native tests for draft survival, stale-key fallback, process cleanup, UTF-16 and IME on the selected Linux toolkit.

Read [ui.md](ui.md) for the shared palette JSON, System/fixed modes, absolute UTF-16 lexical spans and non-mutating native coloring. Use persistent per-document selection/viewport state outside disposable widgets. Visual tab hiding retains Rust content/history and still uses the ten-document budget. Choose the new project's actual main file; refresh dirty metadata even when an agent saves without changing revision. These are reusable contracts, not an implemented Linux UI. Select and qualify native editor/PDF/theme/IME adapters on Linux before claiming parity.


Project macro/ref/BibTeX completion: [project-completion.md](project-completion.md). The shared index is implemented; the new native popup/acceptance remains Mac-only. Port `refresh_project:true` acceptance validation, typed candidates/partial status, whitespace/list/scalar handling and no-op acceptance without history. Shared MSVC type-check does not establish native Windows/Linux behavior.

## Definition navigation to port

Reuse `get_definition` exact-token lookup and its normalized absolute UTF-16 target ranges; see [definition-navigation.md](definition-navigation.md). A future toolkit adapter must convert glyph/widget positions deliberately, use its native Ctrl+click convention, validate target revision/text before opening/selecting and consume navigation outside disposable views. Preserve source viewport/history and guard blank-space/IME/stale hits. Shared code availability is not a Linux build, runtime-directory fix or native gesture qualification.
