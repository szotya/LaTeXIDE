# Windows 11 x64 validation boundary

2026-10-30: no Windows machine or runner is available, as confirmed by the user. This is source and handoff work, **not** a Windows app release.

2026-10-31 repository update: shared Rust tests now execute on GitHub's `windows-2025` runner (initial import: 57 PASS; Unix-only tests differ). This is Windows Server shared-code CI, not WinUI restore/MSBuild/link/launch, Windows 11 or Intel+AMD hardware proof. The native checks below remain required; current repository/CI scope is in [REPOSITORY.md](development/REPOSITORY.md).

Implemented source: `apps/windows/LaTeXIDE.sln` / `.vcxproj`, C++/WinRT App/MainWindow, C ABI ownership bridge, plain-text RichEditBox adapter, shared undo/redo and IME command mapping, local MCP configuration, compiler status and a bounded one-page Windows.Data.Pdf view with page navigation and native zoom.

Pinned direct tools: Rust 1.99.1 MSVC, Microsoft.WindowsAppSDK 2.6.4, Microsoft.Windows.CppWinRT 3.0.260818.1. `tools/windows-nuget-lock.json` and `packages.config` pin 16 packages from official native/universal nuspec dependency groups. `NuGet.Native.props/targets` import their native build assets in dependency order. Metadata inspection is not a successful NuGet restore or MSBuild run.

On a real x64 Visual Studio 2022 developer PowerShell with C++ desktop tools, Windows SDK, official Rust and NuGet available:

```powershell
cargo test --workspace --locked
cargo check --workspace --target x86_64-pc-windows-msvc --locked
.\scripts\build_windows.ps1 -Configuration Debug
.\dist\windows-x64\LaTeXIDE.exe
```

The build script uses only the pinned package list. It expects a separately verified Windows TeX Live tree at `.local/texlive-windows-x64` and stages it beside the app. Follow `tools/texlive/README.md`; Mac's universal-darwin binaries are not Windows binaries. The build script itself has not executed on Windows and may require fixes revealed by the first real build.

Checks to record:

- Linked x64 native build, launch, project/folder picker and high-DPI layout.
- Actual RichEdit paragraph conventions, LF/CRLF round trip, emoji/combining positions, IME commit-once, native undo disabled and Ctrl+Z/Ctrl+Y routed to Rust.
- Named-pipe and descriptor access controls, wrong token, traversal, session shutdown.
- Direct MCP edit → visible change → agent save → GUI undo → save → compile → new PDF; errors keep the old PDF labeled as previous.
- Windows latexmk/Perl/XeLaTeX, fonts/packages and offline template corpus.
- Job Object cancellation/timeout/app-close process-tree cleanup, including children spawned immediately at startup.
- Native PDF page/zoom behavior, Narrator and memory measurements.

The portable `TextAdapter.h` Unicode/paragraph functions were compiled and exercised with Clang on Mac (`evidence/windows-text-adapter.log`). This checks these pure functions, **not** RichEditBox, WinUI/XAML generation, C++/WinRT projections, MSVC linking, named pipes, Windows compiler execution or Windows.Data.Pdf.
