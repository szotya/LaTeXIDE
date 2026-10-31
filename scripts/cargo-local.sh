#!/usr/bin/env bash
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_HOME="$TASK_ROOT/.local/cargo-home"
export RUSTC="$TASK_ROOT/.local/rust/bin/rustc"
export RUSTDOC="$TASK_ROOT/.local/rust/bin/rustdoc"
export PATH="$TASK_ROOT/.local/rust/bin:$PATH"
exec "$TASK_ROOT/.local/rust/bin/cargo" "$@"
