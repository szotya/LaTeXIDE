#!/usr/bin/env python3
"""Check the Git candidate without displaying possible credential values.

This bounded pattern check is a repository hygiene gate, not a security scan.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATTERNS = {
    "github_token": rb"(?:gh[pousr]_[A-Za-z0-9_]{20,}|github_pat_[A-Za-z0-9_]{20,})",
    "api_secret": rb"sk-[A-Za-z0-9_-]{24,}",
    "private_key": rb"-----BEGIN (?:OPENSSH |RSA |EC )?PRIVATE KEY-----",
}
FORBIDDEN_PARTS = {".local", ".codex", "target", "dist", ".latexide", "__pycache__", ".build", "xcuserdata"}
PUBLISHED_EVIDENCE = {
    "evidence/README.md",
    "evidence/ide-audit-2026-10-31/build_audit_artifacts.py",
    "evidence/ide-audit-2026-10-31/validate_audit.py",
    "evidence/ide-audit-2026-10-31/audit_probes.py",
    *{
        "evidence/acquisition/compiler-review/texlive-installer-signatures/" + name
        for name in (
            "texlive.asc", "texlive-keyring.gpg", "install-tl-unx.tar.gz.sha512",
            "install-tl-unx.tar.gz.sha512.asc", "install-tl-windows.exe.sha512",
            "install-tl-windows.exe.sha512.asc",
        )
    },
}
EXECUTABLE_MAGIC = (b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe", b"\xca\xfe\xba\xbe", b"\x7fELF", b"MZ")


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--staged", action="store_true")
    mode.add_argument("--tracked", action="store_true")
    args = parser.parse_args()
    command = ["git", "diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z"] if args.staged else ["git", "ls-files", "-z"]
    names = subprocess.check_output(command, cwd=ROOT).decode().split("\0")
    failures = []
    total_bytes = 0
    for name in filter(None, names):
        path = Path(name)
        if name.startswith("evidence/") and name not in PUBLISHED_EVIDENCE:
            failures.append({"path": name, "reason": "raw_evidence_outside_explicit_public_source_provenance_allowlist"})
            continue
        if FORBIDDEN_PARTS.intersection(path.parts) or path.name == ".env" or (path.name.startswith(".env.") and path.name != ".env.example"):
            failures.append({"path": name, "reason": "local_or_secret_path"})
            continue
        # Read the Git blob, not a potentially different working-tree version.
        revision = f":{name}" if args.staged else f"HEAD:{name}"
        data = subprocess.check_output(["git", "show", revision], cwd=ROOT)
        total_bytes += len(data)
        if len(data) > 5 * 1024 * 1024:
            failures.append({"path": name, "reason": "file_above_5MiB"})
        if data.startswith(EXECUTABLE_MAGIC):
            failures.append({"path": name, "reason": "compiled_executable"})
        for kind, pattern in PATTERNS.items():
            for match in re.finditer(pattern, data):
                failures.append({"path": name, "line": data[:match.start()].count(b"\n") + 1, "reason": kind})
    print(json.dumps({"result": "FAIL" if failures else "PASS", "files": len(list(filter(None, names))), "bytes": total_bytes, "findings": failures,
                      "scope": "bounded credential/path/executable patterns; not a complete security scan"}))
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
