# Windows 11 x64 development reference

Read the repository's `docs/DEVELOPER_HANDOFF.md` first for the latest actual state. The supported target is one x64 release for Intel/AMD Windows 11. Separate x86 and Windows ARM64 products were not requested. There is currently no available Windows runner, so source or Mac-side cross-checks must not be called Windows execution.

## Shared core and native boundary

- Use Rust stable 1.99.1 and the committed/current Cargo.lock. The Mac `.local/rust` bootstrap is an arm64 Mac toolchain, not a Windows installer. Use a real Windows MSVC Rust toolchain for linked Windows builds.
- The shared ABI is `include/latexide_core.h`; link a Windows MSVC Rust static library, not a Mac `.a`. Runtime handles and response buffers retain the same ownership rules.
- Native UI direction is C++/WinRT + WinUI 3. The resolved stable package pins are Windows App SDK 2.6.4 and C++/WinRT 3.0.260818.1. The source project now lives at `apps/windows/LaTeXIDE.sln`; the unexecuted build procedure is `scripts/build_windows.ps1`. `docs/windows-validation.md` lists its exact prerequisites and checks. Native/universal NuGet metadata pins and imports are recorded, but restore/MSBuild remain unrun.
- The initial editor candidate is plain-text RichEditBox behind an adapter. Verify implicit paragraph endings, CRLF mapping, surrogate-pair positions, IME composition, incremental changes and disabling/routing native undo into Rust history.
- The PDF baseline is Windows.Data.Pdf page rendering. Zoom/scroll/page display do not imply native text selection/search or parity with PDFKit.

## Compiler

Use the exact instructions and artifact checks in `tools/texlive/README.md`. The acquired Windows installer has GPG/SHA-512 evidence but no Windows runtime result. Adapt the paths in `tools/texlive/windows-x64.profile` to the actual project root and install the common roots from `package-roots.json` with verification active. `binary_windows` is TeX Live's x64 binary platform.

Keep the local TeX binary folder, its bundled Perl requirements, TEXMF paths and a usable temporary folder explicit in child configuration. Preserve the executable's bin alias rather than replacing it with a resolved script path when constructing PATH. Do not inherit PERL5OPT/PERL5LIB, agent credentials or arbitrary external TeX search paths. The first qualified mode is latexmk/XeLaTeX; other engine modes need their own proof.

## Windows checks to execute

With the target toolchain installed, run the shared Rust tests and a real release build. `cargo check --workspace --target x86_64-pc-windows-msvc --locked` is only a type-check; it neither links WinUI nor runs the app.

Validate the actual named-pipe endpoint, current-user access controls, protected descriptor, wrong-token rejection and runtime shutdown. Exercise Job Object process-tree termination on cancellation, timeout and app exit. These Windows APIs already have source in the Rust tree, but Mac tests do not execute them.

The native acceptance flow is project open → Unicode edit → direct MCP edit → explicit agent save → GUI undo/redo → re-save → compile → fresh PDF. Add stale-revision, external-save-conflict, IME, high-DPI and Narrator checks. Use the project's current test adapters; Python IPC tests currently assume Unix sockets and need a Windows transport branch or equivalent native integration test.

Record exact versions, architecture, build/launch commands, test outputs and screenshots in the handoff/evidence. A failing or unrun step remains visible.

## Native composition and file entries

`list_project_files` returns FileEntry objects with `path` and `size`, not strings. Preserve the shared `begin_composition` / `end_composition` contract: these are native-only commands; while composition is active, mutations/save/history/build return `composition_active`. A valid end commit is one undo transaction; a rejected end also releases composition, and the control must restore acknowledged Rust text before further edits. The current RichEdit source follows these commands but its real event ordering and final paragraph assumptions require Windows tests. Never change the process-wide SIGPIPE policy to mimic Rust main when embedding the core; Mac sets per-socket protection and wakes shutdown without writing.

The shared compiler prefix is `.local/texlive-windows-x64`, matching the profile and staging script. Read `docs/foundation-review.md` for remaining qualification boundaries and the native source fixes.

Mac packaging omits only the optional dangling `man` alias created by the no-doc TeX profile before strict bundle-signature verification. This is a Mac bundle procedure, not a requirement to alter the Windows compiler installation.

## Developer workspace and shared themes

New editor intelligence: [intelligence.md](intelligence.md). Shared `get_completions` type-checks on MSVC; the new native popup and Codex assistant are Mac-only implementations for now. A future Windows adapter must preserve UTF-16 replacement/argument/star handling, stale Enter/Tab behavior, IME, and field-local history versus shared source history. Port stdio process/session/draft contracts with actual Windows APIs; Mac tests do not execute them.

Read [ui.md](ui.md) and `<repo>/docs/windows-ui-port.md`. The source now has the compact developer workspace, shared eight palette variants, independent System/Light/Dark mode, native Find/gutter and bounded Rust foreground coloring. Local structure/portable helper checks pass; MSBuild/XAML/WinUI execution remains unrun. `ProjectFiles.h` resets the main-file field from each newly opened project's actual files, before loading its initial editor document. Verify a project containing only `thesis.tex`, not just `main.tex`.

Preserve selection/viewport through tab switches and visual hiding, metadata-only save refresh, RichEdit exact text/range checks, plain-text asynchronous clipboard guards, IME rollback and AltGr. The filename filter/action menu and page-based PDF are partial parity; do not describe them as a fuzzy popup/searchable command palette/PDFKit text search. Mac controlled renders show the implemented design, not verified Windows pixels.


Project macro/ref/BibTeX completion: [project-completion.md](project-completion.md). The shared index is implemented; the new native popup/acceptance remains Mac-only. Port `refresh_project:true` acceptance validation, typed candidates/partial status, whitespace/list/scalar handling and no-op acceptance without history. Shared MSVC type-check does not establish native Windows/Linux behavior.

## Definition navigation to port

Shared `get_definition` and exact normalized UTF-16 declaration ranges type-check on MSVC x64. The gesture/target adapter currently exists on Mac only. Follow [definition-navigation.md](definition-navigation.md): use native Ctrl+click hit testing, fresh resolve/read/re-resolve, expected-revision target loading and exact selection/scroll. Persist consumed navigation serials outside disposable RichEdit/view instances, preserve prior tab/viewport/undo and test both ordinary clicks and high-DPI glyph/IME boundaries on Windows. Do not replace real WinUI execution with the Mac control tests.
