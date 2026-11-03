# P21 Windows adapter ownership

Read `docs/development/PLATFORM_DEVELOPMENT.md`, the root handoff/skill and `apps/windows/README.md` before changing this tree. Paths in this instruction are repository-relative.

P21 owns the complete `apps/windows/` tree, including existing MainWindow/NativeCore/XAML/project/NuGet files; the 2026-11-03 platform policy supersedes the older blanket P00 ownership of these native files. Shared Rust, C ABI, IPC/MCP, root manifests/locks, shared themes and workflows require a P21 integration request and P00 integration. Do not edit macOS/Linux adapters concurrently from a Windows task.

Use the accepted Mac workflow as the behavior reference and native Windows controls/conventions as the adapter. Preserve single Rust authority, UTF-16/revision checks, IME transaction ownership, field-local keyboard undo, direct agent edits, explicit Save and undo after Save. Keep source/check/link/launch/workflow evidence separate; Windows Server shared CI is not Windows 11 x64 Intel/AMD qualification. Record actual results in `docs/development/results/P21.md` for P00 to carry into handoff/ledger/skill.
