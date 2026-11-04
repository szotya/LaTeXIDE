#!/usr/bin/env python3
"""Scoped developer checks; manifests' test_commands are never executed.

Raw logs/reports belong under ignored .local/, not in repository documentation.
Passing this command is not a release, native UI or online-provider qualification.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform as host_platform
import re
import signal
import shutil
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
_support = importlib.util.spec_from_file_location("latexide_evidence", Path(__file__).with_name("evidence.py"))
evidence = importlib.util.module_from_spec(_support)
_support.loader.exec_module(evidence)
_readiness = importlib.util.spec_from_file_location("latexide_readiness", Path(__file__).with_name("readiness.py"))
readiness = importlib.util.module_from_spec(_readiness)
_readiness.loader.exec_module(readiness)
_p01 = importlib.util.spec_from_file_location("latexide_p01_ci", Path(__file__).with_name("p01_ci.py"))
p01_ci = importlib.util.module_from_spec(_p01)
_p01.loader.exec_module(p01_ci)


def digest(path):
    with Path(path).open("rb") as stream: return hashlib.file_digest(stream, "sha256").hexdigest()


def execute_step(name, argv, cwd, log, timeout, env=None):
    if not argv or not math.isfinite(timeout) or timeout <= 0: raise ValueError("Explicit argv and finite positive timeout required")
    log = Path(log); log.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic(); timed_out = False; reason = None; code = None
    with log.open("wb") as output:
        try:
            child = subprocess.Popen(argv, cwd=cwd, stdout=output, stderr=subprocess.STDOUT, env=env,
                                     start_new_session=os.name != "nt")
            try:
                code = child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                if os.name == "nt":
                    try: subprocess.run(["taskkill", "/PID", str(child.pid), "/T", "/F"], stdout=output, stderr=subprocess.STDOUT, check=False, timeout=5)
                    except (OSError, subprocess.TimeoutExpired): pass
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


def checked_output(root, name, allowed_roots):
    path = Path(root) / name
    if not any(path.resolve().is_relative_to(base) for base in allowed_roots): raise ValueError("Output outside declared directories")
    if not path.is_file(): raise ValueError("Expected output is missing")
    return path


def bounded_json(path):
    with Path(path).open("rb") as stream: raw = stream.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024: raise ValueError("Summary/index quota exceeded")
    return json.loads(raw)


def capture_artifacts(root, step, run_id, allowed_roots):
    specs = list(step.get("artifact_paths", []))
    if step.get("artifact_path"): specs.append({"path": step["artifact_path"], "kind": step.get("artifact_kind", "test_artifact")})
    if step.get("artifact_index"):
        index = bounded_json(checked_output(root, step["artifact_index"], allowed_roots))
        if not isinstance(index, dict) or index.get("schema") != 1 or index.get("verification_run_id") != run_id: raise ValueError("Stale artifact index")
        specs += index.get("artifacts", [])
    if len(specs) > 32: raise ValueError("Artifact quota exceeded")
    artifacts = []; seen = set()
    for spec in specs:
        if not isinstance(spec, dict) or not isinstance(spec.get("path"), str) or not isinstance(spec.get("kind"), str): raise ValueError("Malformed artifact")
        path = checked_output(root, spec["path"], allowed_roots)
        if path.resolve() in seen: raise ValueError("Duplicate artifact identity")
        seen.add(path.resolve())
        artifacts.append({"path": spec["path"], "kind": spec["kind"], "sha256": digest(path)})
    if not set(step.get("required_artifact_kinds", [])).issubset({a["kind"] for a in artifacts}): raise ValueError("Missing required artifact kind")
    return sorted(artifacts, key=lambda a: (a["path"], a["kind"]))


def artifacts_current(root, artifacts, allowed_roots):
    try: return all(digest(checked_output(root, a["path"], allowed_roots)) == a["sha256"] for a in artifacts)
    except (OSError, ValueError): return False


validate_summary = evidence.validate_summary
rust_summary = evidence.rust_summary


def run_checks(root, source_paths, steps, log_dir, timeout, artifact_roots=()):
    root = Path(root).resolve()
    # Only trusted caller-selected generated output roots may live outside the
    # source checkout (e.g. an external build cache). Resolve the allowlist once.
    allowed_roots = [root, *(Path(p).resolve() for p in artifact_roots)]
    before = source_identity(root, source_paths); records = []; source_hashes = {p["path"]: p["sha256"] for p in before["files"]}
    for sequence, step in enumerate(steps, 1):
        run_id = uuid.uuid4().hex; known = {r["name"]: r for r in records}
        blocked = any(known.get(name, {}).get("status") != "passed" for name in step.get("depends_on", []))
        consumed = []; stale = False
        producers = list(step.get("consumes_artifacts", []))
        legacy = step.get("consumes_artifact")
        if legacy: producers.append(legacy["producer"])
        if not blocked:
            for name in sorted(set(producers)):
                producer = known.get(name, {}); selected = producer.get("artifacts", [])
                if legacy and name == legacy["producer"]:
                    selected = [a for a in selected if (root / a["path"]).resolve() == (root / legacy["path"]).resolve()]
                if producer.get("status") != "passed" or not selected: stale = True
                consumed.extend({**a, "producer": name} for a in selected)
            stale = stale or not artifacts_current(root, consumed, allowed_roots)
        if blocked or stale or step.get("argv") is None:
            record = {"name": step["name"], "status": "stale_artifact" if stale and not blocked else "unrun", "exit_code": None, "timed_out": False,
                      "reason": "Dependency did not pass" if blocked else "Artifact no longer matches its producer" if stale else step.get("reason", "Not in this profile"), "artifact_sha256": None}
        else:
            env = dict(os.environ)
            for name, value in step.get("env", {}).items():
                if value is None: env.pop(name, None)
                else: env[name] = value
            env["P00_VERIFICATION_RUN_ID"] = run_id
            p01_tool = None
            p01_tool_error = None
            if step.get("p01_cohort") or step.get("summary_format") == "rust":
                executable = shutil.which(step["argv"][0], path=env.get("PATH"))
                try:
                    if executable is None:
                        raise ValueError("Missing P01 parent build tool")
                    p01_tool = p01_ci.binding(Path(executable).resolve(strict=True))
                except (OSError, ValueError) as error:
                    p01_tool_error = str(error)
            if p01_tool_error:
                record = {"name": step["name"], "command": step["argv"], "status": "unrun",
                          "exit_code": None, "timed_out": False, "reason": p01_tool_error,
                          "artifact_sha256": None}
            else:
                p01_p18_environment = {key: env[key] for key in p01_ci.P18_ENV if key in env}
                record = execute_step(step["name"], step["argv"], root, Path(log_dir) / f"{sequence:02}-{step['name']}.log", timeout, env=env)
                if step.get("summary_format") == "rust":
                    record["rust_build_tool"] = p01_tool
                if step.get("p01_cohort"):
                    record["p18_environment"] = p01_p18_environment
            if record["status"] == "passed":
                try:
                    if step.get("p01_cohort"):
                        spec = step["p01_cohort"]
                        p01_ci.qualify(root, record, step["argv"], run_id, before["sha256"],
                                       root / "target", root / spec["proof"], env, execute_step, p01_tool)
                        record["p01_build_tool"] = p01_tool
                    artifacts = capture_artifacts(root, step, run_id, allowed_roots)
                    if artifacts:
                        record["artifacts"] = artifacts; record["artifact_status"] = "current"
                        record["artifact_sha256"] = artifacts[0]["sha256"] if len(artifacts) == 1 else hashlib.sha256(json.dumps(artifacts, sort_keys=True).encode()).hexdigest()
                        record["artifact_kind"] = artifacts[0]["kind"] if len(artifacts) == 1 else "artifact_set"
                    if step.get("summary_path"):
                        summary = bounded_json(checked_output(root, step["summary_path"], allowed_roots))
                        record["test_summary"] = validate_summary(summary, step["summary_suite"], step.get("allowed_skips", []), run_id)
                    if step.get("summary_format") == "rust":
                        record["test_summary"] = rust_summary(record["log"]["path"], step["name"])
                        if step.get("p01_cohort"):
                            qualification = p01_ci.validate(root, root / step["p01_cohort"]["proof"],
                                record, step["argv"], run_id, before["sha256"], root / "target", p01_tool)
                            record["test_summary"] = p01_ci.combine_summary(record["test_summary"], qualification)
                    if step.get("documentation_path"):
                        data = bounded_json(checked_output(root, step["documentation_path"], allowed_roots))
                        if not isinstance(data, dict) or data.get("verification_run_id") != run_id or not data.get("files"): raise ValueError("Stale or empty documentation identity")
                        for file in data["files"]:
                            if source_hashes.get(file["path"]) != file["sha256"]: raise ValueError("Description is not in the verified source set")
                        record["description_identity"] = data
                except (OSError, ValueError, KeyError, TypeError) as error:
                    record["status"] = "failed"; record["reason"] = str(error)
            if record["status"] != "passed" and record.get("exit_code") is not None:
                try:
                    if step.get("summary_path"):
                        data = bounded_json(checked_output(root, step["summary_path"], allowed_roots))
                        record["test_summary"] = validate_summary(data, step["summary_suite"], run_id=run_id, allow_failures=True)
                    if step.get("summary_format") == "rust": record["test_summary"] = rust_summary(record["log"]["path"], step["name"], allow_failures=True)
                except (OSError, ValueError, KeyError, TypeError) as error: record["summary_error"] = str(error)
            if consumed and not artifacts_current(root, consumed, allowed_roots):
                record["status"] = "stale_artifact"; record["reason"] = "Consumed artifact changed during execution"
        if consumed: record["consumed_artifacts"] = consumed
        record.update({"source_sha256": before["sha256"], "platform": step.get("platform", sys.platform + "/" + host_platform.machine()),
                       "kind": step["kind"], "required": step.get("required", True), "sequence": sequence, "verification_run_id": run_id})
        records.append(record)
    try: after_sha = source_identity(root, source_paths)["sha256"]
    except (OSError, ValueError): after_sha = None
    for record in records:
        if record.get("artifacts") and not artifacts_current(root, record["artifacts"], allowed_roots): record["artifact_status"] = "stale_artifact"
    for step, record in zip(steps, records):
        if step.get("p01_cohort") and record.get("status") == "passed":
            try:
                qualification = p01_ci.validate(root, root / step["p01_cohort"]["proof"], record,
                    step["argv"], record["verification_run_id"], before["sha256"],
                    root / "target", record["p01_build_tool"])
                expected = p01_ci.combine_summary(rust_summary(record["log"]["path"], step["name"]), qualification)
                if not p01_ci.same(expected, record["test_summary"]):
                    raise ValueError("P01 parent summary changed")
            except (OSError, ValueError, KeyError, TypeError) as error:
                record["status"] = "stale_artifact"
                record["reason"] = str(error)
    failed = any(r["required"] and (r["status"] != "passed" or r.get("artifact_status") == "stale_artifact") for r in records)
    status = "source_changed" if before["sha256"] != after_sha else "failed" if failed else "passed"
    for record in records: record["report_status"] = status
    return {"schema": 1, "scope": "P00 selected developer checks; no complete product/platform qualification", "status": status,
            "source_sha256": before["sha256"], "source_sha256_after": after_sha, "source_files": before["files"], "checks": records}


def resolve_evidence(records, source_sha256, platform, kind, *, artifact_sha256=None, capability_id=None):
    candidates = [r for r in records if r.get("platform") == platform and r.get("kind") == kind
                  and (capability_id is None or r.get("capability_id") == capability_id)]
    if not candidates: return "unrun"
    latest = max(enumerate(candidates), key=lambda item: (item[1].get("sequence", 0), item[0]))[1]
    if latest.get("source_sha256") != source_sha256: return "stale_source"
    if artifact_sha256 is not None and latest.get("artifact_sha256") != artifact_sha256: return "stale_artifact"
    if latest.get("artifact_status") == "stale_artifact": return "stale_artifact"
    if latest.get("timed_out") or latest.get("status") == "timed_out": return "timed_out"
    status = latest.get("status", "unrun")
    if status in {"unrun", "skipped", "planned", "missing", "partial", "implemented", "stale_artifact", "stale_source"}: return status
    if latest.get("report_status") == "source_changed": return "stale_source"
    return "passed" if status == "passed" and latest.get("exit_code") == 0 and latest.get("report_status", "passed") == "passed" else "failed"


def discover_sources(root):
    root = Path(root)
    paths = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
    for name in [".gitattributes", ".gitignore"]:
        if (root / name).is_file(): paths.add(name)
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
    for folder in ["crates", "integration/registry", "integration/requests", "tests", "apps/macos", "shared/ide-theme",
                   "include", ".github/workflows", "docs/development/contracts", "skills/latexide-platform-development", "scripts", "tools"]:
        for path in (root / folder).rglob("*"):
            if path.is_file() and not {"target", ".local", "__pycache__", ".build", ".swiftpm"}.intersection(path.relative_to(root).parts) and path.name != ".DS_Store":
                paths.add(path.relative_to(root).as_posix())
    return sorted(paths)


def build_steps(root, args, platform, machine):
    root = Path(root); run_dir = Path(args.run_dir)
    def ref(path): return path.relative_to(root).as_posix()
    def probe(command, *tail): return [sys.executable, "integration/registry/probes.py", command, *map(str, tail)]
    def suite(name, suite, kind, **fields):
        output = run_dir / (name + ".json")
        return {"name": name, "kind": kind, "argv": probe("unittest", "--suite", suite, "--result", output),
                "summary_path": ref(output), "summary_suite": suite, **fields}
    cargo = [args.cargo_executable] if args.cargo_executable else ["cargo", "+1.99.1"]
    offline = ["--offline"] if args.offline else []; target = ["--target-dir", str(root / "target")]
    supported = platform == "darwin" and machine.lower() == "arm64"
    integrated = args.profile == "integrated"
    documentation = run_dir / "documentation.json"
    steps = [
        {"name": "generated-contracts", "argv": [sys.executable, "integration/registry/generate_contracts.py", "--check"], "kind": "source"},
        {"name": "development-plans", "argv": [sys.executable, "scripts/validate_development_packages.py"], "kind": "source"},
        suite("coordinator-contracts", "coordinator", "python-unit"),
        {"name": "documentation", "kind": "documentation", "argv": probe("docs", "--result", documentation), "documentation_path": ref(documentation)},
    ]
    source_dependencies = [s["name"] for s in steps]
    if integrated and supported:
        packages = ["latexide-contracts", "latexide-core", "latexide-protocol", "latexide-build", "latexide-ffi", "latexide-mcp"]
        steps.append({"name": "runtime-clean", "kind": "build-cache", "argv": cargo + ["clean", *target, *[a for p in packages for a in ["-p", p]]],
                      "env": {"CARGO_BUILD_TARGET": None}, "depends_on": source_dependencies})
    rust = "rust-contracts" if args.profile == "contracts" else "rust-workspace"
    if args.profile != "structure":
        steps += [
            {"name": rust, "kind": "rust-unit", "summary_format": "rust", "env": {"CARGO_BUILD_TARGET": None},
             "depends_on": source_dependencies + (["runtime-clean"] if integrated and supported else []),
             "argv": cargo + ["test"] + (["-p", "latexide-contracts"] if args.profile == "contracts" else ["--workspace"]) + ["--locked"] + target + offline + (["--", "--include-ignored", "--test-threads=1"] if supported and args.profile in {"shared", "integrated"} else [])},
            {"name": "isolated-module-seam", "kind": "rust-unit", "summary_format": "rust", "depends_on": [rust], "env": {"CARGO_BUILD_TARGET": None},
             "argv": cargo + ["test", "--manifest-path", "tests/contracts/seam/Cargo.toml", "--locked"] + target + offline},
            {"name": "integration-queue", "kind": "manifest-validation", "depends_on": [rust], "env": {"CARGO_BUILD_TARGET": None},
             "argv": cargo + ["run", "-p", "latexide-contracts", "--bin", "latexide-integration", "--locked"] + target + offline + ["--", "--root", "."]},
        ]
    else: steps.append({"name": "rust-not-selected", "kind": "rust-unit", "argv": None, "required": False})
    if supported and args.profile in {"shared", "integrated"}:
        rust_step = next(step for step in steps if step["name"] == rust)
        rust_step.update(artifact_paths=[{"path": ref(root / "target/debug/latexide-p18-os-probe"), "kind": "p18_os_probe"}],
                         required_artifact_kinds=["p18_os_probe"])
        rust_log = run_dir / f"{next(i for i, step in enumerate(steps, 1) if step['name'] == rust):02}-{rust}.log"
        if p01_ci.catalog(root) is not None:
            at = rust_step["argv"].index("--")
            rust_step["argv"].insert(at, "--message-format=json")
            rust_step["argv"] += p01_ci.skipped_arguments()
            proof = run_dir / "p01-parent-cohorts.json"
            rust_step["p01_cohort"] = {"proof": ref(proof)}
            rust_step["artifact_paths"] += [
                {"path": ref(proof), "kind": "p01_cohort_proof"},
                {"path": ref(rust_log), "kind": "p01_rust_build_log"}]
            rust_step["required_artifact_kinds"] += ["p01_cohort_proof", "p01_rust_build_log"]
        p18_result = run_dir / "p18-pdfs.json"
        steps.append({"name": "p18-compiler-pdfs", "kind": "p18-compiler-pdfs", "depends_on": [rust], "consumes_artifacts": [rust],
                      "argv": [sys.executable, "integration/registry/p18_ci.py", "--rust-log", str(rust_log), "--result", str(p18_result)],
                      "summary_path": ref(p18_result), "summary_suite": "p18-pdfs", "artifact_index": ref(p18_result),
                      "required_artifact_kinds": ["p18_pdf_checker", "p18_compiler_pdf", "p18_pdf_render", "p18_pdf_validation"]})
    if integrated and supported:
        runtime_index = run_dir / "runtime-artifacts.json"; native_index = run_dir / "native-artifacts.json"
        runtime_args = (["--cargo-executable", args.cargo_executable] if args.cargo_executable else []) + offline
        steps += [
            {"name": "runtime-build", "kind": "runtime-build", "depends_on": [rust, "isolated-module-seam", "integration-queue", "p18-compiler-pdfs"],
             "argv": probe("runtime-build", *runtime_args, "--result", runtime_index), "artifact_index": ref(runtime_index),
             "required_artifact_kinds": ["ffi_static", "ffi_dynamic", "mcp_executable"]},
            suite("tool-acquisition", "tools", "python-unit"),
            suite("ipc-roundtrip", "ipc", "ipc-runtime", depends_on=["runtime-build"], consumes_artifacts=["runtime-build"]),
            suite("mcp-roundtrip", "mcp", "mcp-runtime", depends_on=["runtime-build"], consumes_artifacts=["runtime-build"]),
            {"name": "native-build", "kind": "native-control-build", "depends_on": ["runtime-build"], "consumes_artifacts": ["runtime-build"],
             "argv": probe("native-build", "--result", native_index), "artifact_index": ref(native_index), "required_artifact_kinds": ["native_test_binary", "native_theme_resource"]},
        ]
        native_summary = run_dir / "native-controls.json"
        steps.append({"name": "native-controls", "kind": "native-controls", "depends_on": ["native-build"], "consumes_artifacts": ["runtime-build", "native-build"],
                      "argv": probe("native-controls", "--result", native_summary), "summary_path": ref(native_summary), "summary_suite": "native-controls",
                      "allowed_skips": ["AssistantSessionTests.testInstalledCodexInitializeOnly", "IntelligenceRenderTests.testRenderAssistantAndCompletionControls",
                                        "ProjectCompletionRenderTests.testRenderProjectMacroReferenceAndCitationCandidates", "WorkspaceRenderTests.testRenderPaletteWorkspaces"]})
    elif integrated:
        for name, kind in [("runtime-build", "runtime-build"), ("ipc-roundtrip", "ipc-runtime"), ("mcp-roundtrip", "mcp-runtime"),
                           ("native-build", "native-control-build"), ("native-controls", "native-controls")]:
            steps.append({"name": name, "kind": kind, "argv": None, "required": True, "platform": "darwin/arm64", "reason": "The integrated adapter requires macOS arm64; use shared for common Rust"})
    schema_selected = args.native_schema or integrated
    if schema_selected and supported:
        native = run_dir / "contract-native"; summary = run_dir / "schema-summary.json"
        steps += [
            {"name": "native-schema-build", "kind": "native-schema-build", "depends_on": source_dependencies, "artifact_path": ref(native), "artifact_kind": "native_schema_test_binary",
             "argv": ["swiftc", "-module-cache-path", str(root / ".local/clang-module-cache"), "tests/contracts/NativeModels.swift", "tests/contracts/main.swift", "-o", str(native)]},
            {"name": "native-schema-decode", "kind": "native-schema", "depends_on": ["native-schema-build"], "consumes_artifacts": ["native-schema-build"],
             "argv": probe("native-schema", "--binary", native, "--result", summary), "summary_path": ref(summary), "summary_suite": "native-schema"},
        ]
    else: steps.append({"name": "native-schema-unavailable" if schema_selected else "native-schema-not-selected", "kind": "native-schema", "argv": None,
                       "required": schema_selected, "reason": "Requires macOS arm64" if schema_selected else "Not requested"})
    for name in ["windows11-winui", "linux-desktop", "live-packaged-app", "online-model", "texlive-runtime", "release-performance", "accessibility"]:
        steps.append({"name": name, "kind": name, "argv": None, "required": False, "reason": "Separate product/platform acceptance; not run by P00"})
    return steps


def requirement_inventory(root, report):
    root = Path(root)
    audit_path = root / "docs/ide-audit-2026-10-31/backlog.json"
    audit = bounded_json(audit_path)
    owners = {r["requirement_id"]: r["package_id"] for r in bounded_json(root / "integration/registry/owners.v1.json")["requirements"]}
    by_name = {r["name"]: r for r in report["checks"]}; inventory = {}
    bindings = {"QA-08": ["rust-workspace", "integration-queue", "runtime-build", "ipc-roundtrip", "mcp-roundtrip", "native-controls"],
                "QA-14": ["documentation", "integration-queue"], "OS-10": ["documentation"],
                "LANG-06": ["rust-workspace", "ipc-roundtrip", "native-controls"]}
    audit_sha = digest(audit_path)
    for item in audit:
        selected = [by_name.get(name) for name in bindings.get(item["id"], [])]
        current = "unrun"
        if any(selected):
            current = "passed" if report["status"] == "passed" and all(r and r["status"] == "passed" for r in selected) else "partial"
            if report["status"] == "source_changed": current = "stale_source"
        inventory[item["id"]] = {"owner": owners[item["id"]],
            "baseline": {"observed_status": item["observed_status"], "audit_sha256": audit_sha, "scope": "historical audit; not fresh acceptance"},
            "acceptance": "partial" if item["observed_status"] == "partial" or item["id"] in {"QA-08", "QA-14", "OS-10"} else "unqualified",
            "unqualified_prerequisites": item["prerequisites"],
            "current_verification": {"status": current, "source_sha256": report["source_sha256"],
                                     "checks": [r["name"] for r in selected if r], "scope": "selected regressions; no provider/release acceptance"}}
    return inventory


def write_report(path, data):
    path = Path(path); path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", newline="\n", dir=path.parent, prefix=".report-", delete=False) as output:
            temporary = Path(output.name); json.dump(data, output, ensure_ascii=False, indent=2); output.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None and temporary.exists(): temporary.unlink()


def repository_identity(root):
    try:
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, stderr=subprocess.DEVNULL, text=True, timeout=5).strip()
        dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "-z"], cwd=root, stderr=subprocess.DEVNULL, timeout=5))
        return {"head": head, "dirty": dirty}
    except (OSError, subprocess.SubprocessError): return {"head": None, "dirty": None}


def recheck_report(path, *, cargo_executable=None, offline=False):
    try:
        report = evidence.read_json(path)
        current = source_identity(ROOT, lambda: discover_sources(ROOT))
        # A stale source set takes precedence over interpreting old metadata.
        if report.get("source_sha256") != current["sha256"]:
            validation = evidence.revalidate_report(ROOT, report, current, [])
            return {**validation, "capability_inventory": readiness.capability_inventory(ROOT),
                    "p00_readiness": readiness.p00_readiness(ROOT, report, validation)}
        selection = report["selection"]
        if report["profile"] not in {"structure", "contracts", "shared", "integrated"} or type(selection["native_schema"]) is not bool:
            raise ValueError("Unknown check selection")
        if selection["platform"] != sys.platform or selection["machine"].lower() != host_platform.machine().lower():
            raise ValueError("Recheck requires the original platform")
        relative = report["run_directory"]
        if not isinstance(relative, str) or "\\" in relative or Path(relative).is_absolute() or ".." in Path(relative).parts or not relative.startswith(".local/"):
            raise ValueError("Invalid run directory")
        if type(offline) is not bool or (cargo_executable is not None and
                (type(cargo_executable) is not str or not cargo_executable)):
            raise ValueError("Invalid caller-selected Cargo/offline configuration")
        # Selection comes from the trusted caller, never a stored argv. Recheck
        # only reconstructs fixed checks and reads their original observations.
        args = argparse.Namespace(profile=report["profile"], native_schema=selection["native_schema"],
                                  cargo_executable=cargo_executable, offline=offline, run_dir=ROOT / relative)
        steps = build_steps(ROOT, args, selection["platform"], selection["machine"])
        if len(steps) != len(report["checks"]):
            raise ValueError("Stored check selection changed")
        for step, row in zip(steps, report["checks"]):
            if step.get("argv") is None:
                continue
            if not p01_ci.same(row.get("command"), step["argv"]):
                raise ValueError("Recheck requires the original caller-selected command/tool/offline configuration")
            if step.get("summary_format") == "rust":
                executable = shutil.which(step["argv"][0])
                if executable is None or not p01_ci.same(row.get("rust_build_tool"),
                        p01_ci.binding(Path(executable).resolve(strict=True))):
                    raise ValueError("Original Rust build tool is missing or changed")
        updated = finalize_report(ROOT, report, steps, artifact_roots=[ROOT / "target", ROOT / ".local/p00-native-build"])
        return {**updated["evidence_validation"], "capability_inventory": updated.get("capability_inventory", {}),
                "p00_readiness": updated.get("p00_readiness", {})}
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        return {"schema": 1, "status": "invalid_report", "reason": str(error), "scope": "read-only freshness; rerun verification"}


def finalize_report(root, report, steps, artifact_roots=()):
    """Derive progress from freshly checked bytes, then guard late input changes."""
    try:
        current = source_identity(root, lambda: discover_sources(root))
        validation = evidence.revalidate_report(root, report, current, steps, artifact_roots)
        report["evidence_validation"] = validation
        report["capability_inventory"] = readiness.capability_inventory(root)
        report["p00_readiness"] = readiness.p00_readiness(root, report, validation)
        if report["status"] == "passed" and validation["status"] != "current":
            report["status"] = "failed"
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        report["status"] = "failed"; report["readiness_error"] = str(error)
        report["evidence_validation"] = {"schema": 1, "status": "invalid_report", "reason": str(error)}
        report["capability_inventory"] = {}; report["p00_readiness"] = {}
    try: after = source_identity(root, lambda: discover_sources(root))["sha256"]
    except (OSError, ValueError): after = None
    if report["status"] == "passed" and after == report.get("source_sha256"):
        # Inventory construction is read-only but may overlap another build.
        validation = evidence.revalidate_report(root, report, source_identity(root, lambda: discover_sources(root)), steps, artifact_roots)
        report["evidence_validation"] = validation
        if validation["status"] != "current":
            report["status"] = "failed"
            progress = report.get("p00_readiness", {})
            progress["evidence_status"] = validation["status"]; progress["independent_status"] = "partial"
            for row in progress.get("requirements", {}).values():
                for criterion in row["criteria"]:
                    if criterion["status"] == "passed": criterion["status"] = validation["status"]
    # Rehashing the earlier list cannot detect additions during that last pass.
    try: after = source_identity(root, lambda: discover_sources(root))["sha256"]
    except (OSError, ValueError): after = None
    if report.get("source_sha256") is not None and after != report["source_sha256"]:
        report["status"] = "source_changed"; report["source_sha256_after"] = after
        validation = report["evidence_validation"]
        validation.update(status="stale_source", source_sha256=after, reason="Source set changed during report finalization")
        validation.setdefault("identities", {})["source"] = "stale_source"
        report["capability_inventory"]["freshness"] = "stale_source"
        progress = report.get("p00_readiness", {})
        progress["evidence_status"] = "stale_source"; progress["independent_status"] = "partial"
        for row in progress.get("requirements", {}).values():
            for criterion in row["criteria"]:
                if criterion["status"] == "passed": criterion["status"] = "stale_source"
    if report["status"] != "passed":
        for requirement in report.get("requirements", {}).values():
            if requirement["current_verification"]["checks"]:
                requirement["current_verification"]["status"] = "stale_source" if report["status"] == "source_changed" else "failed"
    for record in report["checks"]: record["report_status"] = report["status"]
    return report


def failure_diagnostics(records):
    """Bounded synthetic Python-test tails for actionable CI failure output."""
    details = []
    for row in records:
        if row.get("name") not in {"coordinator-contracts", "tool-acquisition"} or row.get("status") == "passed" or not row.get("log"):
            continue
        try:
            path = Path(row["log"]["path"])
            with path.open("rb") as stream:
                stream.seek(max(0, path.stat().st_size - 8192))
                tail = stream.read(8192).decode("utf-8", errors="replace")
            details.append({"name": row["name"], "tail": tail})
        except OSError as error:
            details.append({"name": row["name"], "reason": str(error)})
    return details


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=["structure", "contracts", "shared", "integrated"], default="contracts")
    parser.add_argument("--recheck-report", type=Path, help="Read-only revalidation of stored evidence; no commands run")
    parser.add_argument("--cargo-executable", help="Explicit local cargo binary; otherwise use pinned rustup toolchain")
    parser.add_argument("--offline", action="store_true"); parser.add_argument("--native-schema", action="store_true")
    parser.add_argument("--timeout", type=float, default=600)
    parser.add_argument("--output", type=Path, default=ROOT / ".local/p00-verification/report.json")
    args = parser.parse_args()
    if not math.isfinite(args.timeout) or args.timeout <= 0: parser.error("timeout must be finite and positive")
    if args.recheck_report:
        result = recheck_report(args.recheck_report, cargo_executable=args.cargo_executable, offline=args.offline)
        # JSON stdout stays UTF-8-compatible even on redirected Windows cp1252.
        print(json.dumps(result, ensure_ascii=True))
        return 0 if result["status"] == "current" else 1
    verification_id = uuid.uuid4().hex
    write_report(args.output, {"schema": 1, "status": "running", "profile": args.profile, "verification_id": verification_id})
    cache = ROOT / ".local/p00-verification"; cache.mkdir(parents=True, exist_ok=True)
    args.run_dir = Path(tempfile.mkdtemp(prefix="run-", dir=cache))
    (ROOT / ".local/clang-module-cache").mkdir(parents=True, exist_ok=True)
    steps = build_steps(ROOT, args, sys.platform, host_platform.machine())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    try:
        report = run_checks(ROOT, lambda: discover_sources(ROOT), steps, args.run_dir, args.timeout,
                            artifact_roots=[ROOT / "target", ROOT / ".local/p00-native-build"])
    except (OSError, ValueError, KeyError, TypeError) as error:
        report = {"schema": 1, "status": "failed", "reason": str(error), "source_sha256": None, "source_sha256_after": None, "source_files": [], "checks": []}
    report["profile"] = args.profile
    report["selection"] = {"native_schema": args.native_schema, "platform": sys.platform, "machine": host_platform.machine()}
    report["run_directory"] = args.run_dir.relative_to(ROOT).as_posix()
    report["verification_id"] = verification_id
    report["repository"] = repository_identity(ROOT)
    try: report["requirements"] = requirement_inventory(ROOT, report)
    except (OSError, ValueError, KeyError, TypeError) as error:
        report["status"] = "failed"; report["requirements"] = {}; report["inventory_error"] = str(error)
    report["description_identity"] = [r["description_identity"] for r in report["checks"] if r.get("description_identity")]
    report = finalize_report(ROOT, report, steps, artifact_roots=[ROOT / "target", ROOT / ".local/p00-native-build"])
    write_report(args.output, report)
    print(json.dumps({"profile": args.profile, "status": report["status"], "source_sha256": report["source_sha256"],
                      "checks": [{"name": r["name"], "status": r["status"], "kind": r["kind"], "tests": r.get("test_summary")} for r in report["checks"]],
                      "failure_diagnostics": failure_diagnostics(report["checks"])}, ensure_ascii=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__": raise SystemExit(main())
