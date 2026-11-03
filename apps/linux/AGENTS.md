# P22 Linux adapter ownership

Read `docs/development/PLATFORM_DEVELOPMENT.md`, the root handoff/skill and `apps/linux/README.md` before changing this tree. Paths in this instruction are repository-relative.

P22 owns all of `apps/linux/`, not only `platform_probe/`, plus its native test tree and future `scripts/build_linux.sh`. A self-contained app/probe Cargo manifest and lockfile can live here; registering it in the root workspace remains P00 work. Shared Rust/FFI/session/IPC/C ABI/MCP, root locks, themes and workflows require a P22 integration request. Other platform adapters are read-only for a Linux task.

Mac defines accepted behavior, not Linux filesystem paths or widget APIs. There is no selected Linux UI toolkit yet. Keep toolkit choice explicit, validate real XDG/runtime ownership/mode/symlink state, preserve UTF-16/revisions and one Rust document/undo authority. No automatic `/private/tmp` fallback. Keep Wayland/X11, shared unit tests and actual desktop/IME/PDF/MCP workflows separate. Record evidence in `docs/development/results/P22.md`; P00 updates shared handoff/ledger/skill from it.
