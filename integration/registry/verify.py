#!/usr/bin/env python3
"""Scoped developer checks; manifests' test_commands are never executed.

Raw logs/reports belong under ignored .local/, not in repository documentation.
Passing this command is not a release, native UI or online-provider qualification.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform as host_platform
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with Path(path).open("rb") as stream: return hashlib.file_digest(stream, "sha256").hexdigest()


def execute_step(name, argv, cwd, log, timeout):
    if not argv or timeout <= 0: raise ValueError("Explicit argv and positive timeout required")
    log = Path(log); log.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic(); timed_out = False; reason = None; code = None
    with log.open("wb") as output:
        try:
            child = subprocess.Popen(argv, cwd=cwd, stdout=output, stderr=subprocess.STDOUT,
                                     start_new_session=os.name != "nt")
            try:
                code = child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                if os.name == "nt":
                    subprocess.run(["taskkill", "/PID", str(child.pid), "/T", "/F"], stdout=output, stderr=subprocess.STDOUT, check=False)
                    if child.poll() is None: child.kill()
                else:
                    try: os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError: pass
                code = child.wait(timeout=5)
        except OSError as error:
            reason = str(error)
    status = "timed_out" if timed_out else "unrun" if reason else "passed" if code == 0 else "failed"
    return {"name": name, "command": list(argv), "status": status, "exit_code": code,
            "timed_out": timed_out, "duration_ms": round((time.monotonic() - started) * 1000),
            "reason": reason, "artifact_sha256": None,
            "log": {"kind": "command_log", "path": str(log), "sha256": digest(log)}}


def source_identity(root, paths):
    root = Path(root).resolve(); files = []
    for name in sorted(set(paths() if callable(paths) else paths)):
        path = root / name
        if Path(name).is_absolute() or not path.resolve().is_relative_to(root) or ".." in Path(name).parts:
            raise ValueError("Source path outside the declared root")
        files.append({"path": name, "sha256": digest(path), "bytes": path.stat().st_size})
    if not files: raise ValueError("An explicit source set is required")
    canonical = json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    return {"sha256": hashlib.sha256(canonical).hexdigest(), "files": files}


def run_checks(root, source_paths, steps, log_dir, timeout):
    before = source_identity(root, source_paths); records = []
    for sequence, step in enumerate(steps, 1):
        known = {r["name"]: r for r in records}
        blocked = any(known.get(name, {}).get("status") != "passed" for name in step.get("depends_on", []))
        stale_artifact = False
        if step.get("consumes_artifact") and not blocked:
            artifact = step["consumes_artifact"]
            producer = known.get(artifact["producer"], {})
            path = Path(root) / artifact["path"]
            stale_artifact = not path.is_file() or not producer.get("artifact_sha256") or digest(path) != producer["artifact_sha256"]
        if blocked or stale_artifact or step.get("argv") is None:
            record = {"name": step["name"], "status": "unrun", "exit_code": None, "timed_out": False,
                      "reason": "Dependency did not pass" if blocked else "Artifact no longer matches its producer" if stale_artifact else step.get("reason", "Not in this profile"), "artifact_sha256": None}
            if stale_artifact: record["status"] = "stale_artifact"
        else:
            record = execute_step(step["name"], step["argv"], root, Path(log_dir) / f"{sequence:02}-{step['name']}.log", timeout)
            if step.get("artifact_path") and record["status"] == "passed":
                path = Path(root) / step["artifact_path"]
                if path.is_file() and path.resolve().is_relative_to(Path(root).resolve()):
                    record["artifact_sha256"] = digest(path)
                    record["artifact_kind"] = step.get("artifact_kind", "test_artifact")
                else:
                    record["status"] = "failed"; record["reason"] = "Expected artifact is missing or outside source root"
        record.update({"source_sha256": before["sha256"], "platform": sys.platform + "/" + host_platform.machine(),
                       "kind": step["kind"], "required": step.get("required", True), "sequence": sequence})
        records.append(record)
    try:
        after = source_identity(root, source_paths); after_sha = after["sha256"]
    except (OSError, ValueError): after_sha = None
    status = "source_changed" if before["sha256"] != after_sha else "failed" if any(r["required"] and r["status"] != "passed" for r in records) else "passed"
    return {"schema": 1, "scope": "P00 developer checks; no product/platform qualification", "status": status,
            "source_sha256": before["sha256"], "source_sha256_after": after_sha, "source_files": before["files"], "checks": records}


def resolve_evidence(records, source_sha256, platform, kind, *, artifact_sha256=None, capability_id=None):
    candidates = [r for r in records if r.get("platform") == platform and r.get("kind") == kind
                  and (capability_id is None or r.get("capability_id") == capability_id)]
    if not candidates: return "unrun"
    latest = max(enumerate(candidates), key=lambda item: (item[1].get("sequence", 0), item[0]))[1]
    if latest.get("source_sha256") != source_sha256: return "stale_source"
    if artifact_sha256 is not None and latest.get("artifact_sha256") != artifact_sha256: return "stale_artifact"
    if latest.get("timed_out") or latest.get("status") == "timed_out": return "timed_out"
    status = latest.get("status", "unrun")
    if status in {"unrun", "skipped", "planned", "missing", "partial", "implemented", "stale_artifact", "stale_source"}: return status
    return "passed" if status == "passed" and latest.get("exit_code") == 0 else "failed"


def discover_sources(root):
    root = Path(root)
    paths = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
    if (root / ".gitattributes").is_file(): paths.add(".gitattributes")
    for name in ["docs/development/package-specs.json", "docs/development/packages.json",
                 "docs/ide-audit-2026-10-31/backlog.json", "scripts/build_development_packages.py",
                 "scripts/validate_development_packages.py"]:
        if (root / name).is_file(): paths.add(name)
    # The planning validator consumes these outputs/docs as well as its JSON
    # inputs. Keep result journals separate: they report a run, not feed it.
    for folder, pattern in [("docs/development", "*.json"), ("docs/development", "*.csv"),
                            ("docs/development", "*.md"),
                            ("docs/superpowers/plans/2026-10-31-parallel", "*.md")]:
        paths.update(path.relative_to(root).as_posix() for path in (root / folder).glob(pattern) if path.is_file())
    spec = root / "docs/superpowers/specs/2026-10-31-parallel-development-design.md"
    if spec.is_file(): paths.add(spec.relative_to(root).as_posix())
    for folder in ["crates", "integration/registry", "integration/requests", "tests/contracts"]:
        for path in (root / folder).rglob("*"):
            if path.is_file() and not {"target", ".local", "__pycache__"}.intersection(path.relative_to(root).parts) and path.suffix in {".rs", ".py", ".swift", ".toml", ".json", ".lock"}:
                paths.add(path.relative_to(root).as_posix())
    return sorted(paths)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=["structure", "contracts", "shared"], default="contracts")
    parser.add_argument("--cargo-executable", help="Explicit local cargo binary; otherwise use rustup's pinned toolchain")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--native-schema", action="store_true")
    parser.add_argument("--timeout", type=float, default=600)
    parser.add_argument("--output", type=Path, default=ROOT / ".local/p00-verification/report.json")
    args = parser.parse_args()
    cargo = [args.cargo_executable] if args.cargo_executable else ["cargo", "+1.99.1"]
    offline = ["--offline"] if args.offline else []
    steps = [
        {"name": "generated-contracts", "argv": [sys.executable, "integration/registry/generate_contracts.py", "--check"], "kind": "source"},
        {"name": "development-plans", "argv": [sys.executable, "scripts/validate_development_packages.py"], "kind": "source"},
        {"name": "coordinator-contracts", "argv": [sys.executable, "-m", "unittest", "discover", "-s", "tests/contracts", "-p", "test_*.py"], "kind": "python-unit"},
    ]
    if args.profile != "structure":
        steps += [
            {"name": "rust-contracts" if args.profile == "contracts" else "rust-workspace", "kind": "rust-unit",
             "argv": cargo + ["test"] + (["-p", "latexide-contracts"] if args.profile == "contracts" else ["--workspace"]) + ["--locked"] + offline},
            {"name": "isolated-module-seam", "kind": "rust-unit", "argv": cargo + ["test", "--manifest-path", "tests/contracts/seam/Cargo.toml", "--locked"] + offline},
        ]
    else: steps.append({"name": "rust-not-selected", "kind": "rust-unit", "argv": None, "required": False})
    if args.native_schema and sys.platform == "darwin":
        (ROOT / ".local/clang-module-cache").mkdir(parents=True, exist_ok=True)
        (ROOT / ".local/p00-verification").mkdir(parents=True, exist_ok=True)
        native = Path(tempfile.mkdtemp(prefix="native-schema-", dir=ROOT / ".local/p00-verification")) / "contract-native"
        artifact_path = native.relative_to(ROOT).as_posix()
        steps += [
            {"name": "native-schema-build", "kind": "native-schema-build", "artifact_path": artifact_path, "artifact_kind": "native_schema_test_binary", "argv": ["swiftc", "-module-cache-path", str(ROOT / ".local/clang-module-cache"), "tests/contracts/NativeModels.swift", "tests/contracts/main.swift", "-o", str(native)]},
            {"name": "native-schema-decode", "kind": "native-schema", "depends_on": ["native-schema-build"], "consumes_artifact": {"producer": "native-schema-build", "path": artifact_path}, "argv": [str(native), str(ROOT / "tests/contracts/v1")]},
        ]
    else: steps.append({"name": "native-schema-unavailable" if args.native_schema else "native-schema-not-selected", "kind": "native-schema", "argv": None,
                       "required": args.native_schema, "reason": "Swift native schema check requires macOS" if args.native_schema else "Not requested"})
    for name in ["windows11-winui", "linux-desktop", "live-packaged-app", "online-model", "texlive-runtime", "release-performance", "accessibility"]:
        steps.append({"name": name, "kind": name, "argv": None, "required": False, "reason": "Separate product/platform acceptance; not run by P00"})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = run_checks(ROOT, lambda: discover_sources(ROOT), steps, args.output.parent, args.timeout)
    report["profile"] = args.profile
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"profile": args.profile, "status": report["status"], "source_sha256": report["source_sha256"],
                      "checks": [{"name": r["name"], "status": r["status"], "kind": r["kind"]} for r in report["checks"]]}, ensure_ascii=False))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__": raise SystemExit(main())
