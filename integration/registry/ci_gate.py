#!/usr/bin/env python3
"""Required CI decision; unknown/missing work fails closed, no repository writes."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess


def requires_runtime(paths: list[str]) -> bool:
    def documentation(path):
        return path == "README.md" or (path.endswith(".md") and path.startswith(("docs/", "skills/")))
    return any(not documentation(path) for path in paths)


def gate_succeeded(needs: dict) -> bool:
    if not isinstance(needs, dict): return False
    try:
        required = needs["changes"]["outputs"]["runtime"]
        return (needs["planning"]["result"] == "success" and needs["changes"]["result"] == "success"
                and required in {"true", "false"}
                and needs["runtime"]["result"] in ({"success"} if required == "true" else {"skipped", "success"}))
    except (KeyError, TypeError): return False


def changed_paths(root, base, head):
    if not head or not re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", head): raise ValueError("invalid_git_range")
    subprocess.run(["git", "rev-parse", "--verify", head + "^{commit}"], cwd=root, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    if not base or set(base) == {"0"}: return None
    if not re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", base): raise ValueError("invalid_git_range")
    # Disable rename collapsing: removing code while adding a docs path still
    # changes code, even when Git would classify it as a content-identical move.
    raw = subprocess.check_output(["git", "diff", "--name-only", "--no-renames", "-z", base, head, "--"], cwd=root, stderr=subprocess.DEVNULL, timeout=30)
    if len(raw) > 16 * 1024 * 1024: raise ValueError("diff_quota")
    return [p.decode("utf-8") for p in raw.split(b"\0") if p]


def main():
    parser = argparse.ArgumentParser(); commands = parser.add_subparsers(dest="command", required=True)
    change = commands.add_parser("changes")
    change.add_argument("--root", type=Path, default=Path.cwd())
    change.add_argument("--base", default=os.environ.get("P00_CI_BASE"))
    change.add_argument("--head", default=os.environ.get("P00_CI_HEAD"))
    commands.add_parser("aggregate")
    args = parser.parse_args()
    try:
        if args.command == "changes":
            paths = changed_paths(args.root, args.base, args.head)
            required = paths is None or requires_runtime(paths)
            if os.environ.get("GITHUB_OUTPUT"):
                with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8", newline="\n") as output:
                    output.write("runtime=" + str(required).lower() + "\n")
            result = {"status": "passed", "runtime_required": required, "changed_path_count": None if paths is None else len(paths)}
        else:
            result = {"status": "passed" if gate_succeeded(json.loads(os.environ.get("P00_CI_NEEDS", "{}"))) else "failed"}
    except (ValueError, OSError, subprocess.SubprocessError):
        result = {"status": "failed", "reason": "Invalid or unavailable CI inputs"}
    print(json.dumps(result))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__": raise SystemExit(main())
