#!/usr/bin/env bash
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-run}"
# Preserve any unsaved workspace. Quit normally before rebuilding a running instance.
if pgrep -x LaTeXIDE >/dev/null; then
  echo 'Close or save the existing LaTeX IDE before rebuilding.' >&2; exit 1
fi
bash "$TASK_ROOT/scripts/build_macos.sh"
TASK_APP="$TASK_ROOT/dist/LaTeXIDE.app"
case "$MODE" in
  run) /usr/bin/open -n "$TASK_APP" ;;
  --verify) /usr/bin/open -n "$TASK_APP"; sleep 1; pgrep -x LaTeXIDE ;;
  --debug) /usr/bin/open -n "$TASK_APP"; sleep 1; lldb -n LaTeXIDE ;;
  --logs|--telemetry) /usr/bin/open -n "$TASK_APP"; /usr/bin/log stream --info --style compact --predicate 'process == "LaTeXIDE"' ;;
  *) echo 'usage: script/build_and_run.sh [run|--verify|--debug|--logs|--telemetry]' >&2; exit 2 ;;
esac
