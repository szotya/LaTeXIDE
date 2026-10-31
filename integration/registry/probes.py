#!/usr/bin/env python3
"""Adapters for fixed developer suites/builders; no manifest command is executed."""
import argparse
import hashlib
import importlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
DOCUMENTS = ["docs/development/contracts/p00-closure.md", "docs/development/contracts/p00-readiness-spec.md",
             "docs/development/contracts/p00-readiness-plan.md", "docs/development/contracts/v1.md", "docs/development/contracts/p00-integrations-plan.md",
             "skills/latexide-platform-development/SKILL.md",
             "skills/latexide-platform-development/references/parallel-development.md"]


def write_json(path, data):
    path = Path(path); path.parent.mkdir(parents=True, exist_ok=True)
    if os.environ.get("P00_VERIFICATION_RUN_ID"): data = {**data, "verification_run_id": os.environ["P00_VERIFICATION_RUN_ID"]}
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


class OutcomeResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.passed_cases = 0; self.failed_cases = set(); self.started_cases = {}; self.skipped_cases = 0

    def startTest(self, test):
        super().startTest(test); self.started_cases[id(test)] = self.testsRun

    def stopTest(self, test):
        self.started_cases.pop(id(test), None); super().stopTest(test)

    def mark_failed(self, test):
        if id(test) in self.started_cases: self.failed_cases.add(self.started_cases[id(test)])

    def addSuccess(self, test):
        self.passed_cases += 1; super().addSuccess(test)

    def addFailure(self, test, error):
        self.mark_failed(test); super().addFailure(test, error)

    def addError(self, test, error):
        self.mark_failed(test); super().addError(test, error)

    def addSubTest(self, test, subtest, error):
        if error is not None: self.mark_failed(test)
        super().addSubTest(test, subtest, error)

    def addSkip(self, test, reason):
        if id(test) in self.started_cases: self.skipped_cases += 1
        super().addSkip(test, reason)

    def addUnexpectedSuccess(self, test):
        self.mark_failed(test); super().addUnexpectedSuccess(test)


def run_unittest(suite, name, result_path, stream=None):
    result = unittest.TextTestRunner(stream=stream or sys.stderr, verbosity=2, resultclass=OutcomeResult).run(suite)
    summary = {"schema": 1, "suite": name, "tests": result.testsRun,
               "passed": result.passed_cases, "failed_tests": len(result.failed_cases), "skipped_tests": result.skipped_cases,
               "expected_failures": len(result.expectedFailures), "unexpected_successes": len(result.unexpectedSuccesses),
               "skipped": len(result.skipped), "failures": len(result.failures), "errors": len(result.errors),
               "skipped_ids": [case.id() for case, _ in result.skipped]}
    write_json(result_path, summary)
    return 0 if result.wasSuccessful() and result.testsRun > 0 else 1


def parse_native_summary(text):
    matches = list(re.finditer(r"Test Suite 'All tests' (?:passed|failed)[^\n]*\n\s*Executed (\d+) tests?, with (?:(\d+) tests? skipped and )?(\d+) failures?", text))
    if not matches: raise ValueError("Missing All tests XCTest summary")
    count, skipped, failures = matches[-1].groups(); count, skipped, failures = int(count), int(skipped or 0), int(failures)
    ids = set(); failed_ids = set()
    for line in text.splitlines():
        match = re.search(r"-\[(?:\w+\.)?(\w+) (\w+)\]", line)
        if not match: continue
        identifier = match[1] + "." + match[2]
        if "Test skipped" in line or "' skipped" in line: ids.add(identifier)
        if "Test Case" in line and "' failed" in line: failed_ids.add(identifier)
    if count <= 0 or skipped != len(ids) or skipped + len(failed_ids) > count or (failures > 0 and not failed_ids): raise ValueError("Inconsistent XCTest summary")
    return {"schema": 1, "suite": "native-controls", "framework": "XCTest", "tests": count, "passed": count - skipped - len(failed_ids),
            "failed_tests": len(failed_ids), "skipped_tests": skipped,
            "skipped": skipped, "failures": failures, "errors": 0, "skipped_ids": sorted(ids)}


def check_documents(root, names):
    root = Path(root).resolve(); files = []; links = 0
    for name in names:
        path = root / name
        if not path.resolve().is_relative_to(root): raise ValueError("Description outside repository")
        raw = path.read_bytes(); text = re.sub(r"```.*?```", "", raw.decode("utf-8"), flags=re.S)
        for target in re.findall(r"\]\(([^)]+)\)", text):
            target = target.split("#", 1)[0].strip("<>")
            if not target or re.match(r"[a-z][a-z0-9+.-]*:", target): continue
            resolved = path.parent / target
            if Path(target).is_absolute() or not resolved.resolve().is_relative_to(root) or not resolved.exists():
                raise ValueError("Broken or external local description link")
            links += 1
        files.append({"path": name, "sha256": hashlib.sha256(raw).hexdigest()})
    return {"schema": 1, "scope": "local guide/skill links and bytes; not prose or product acceptance", "checked_links": links, "files": files}


def command_result(argv, env=None):
    result = subprocess.run(argv, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
    sys.stdout.write(result.stdout); sys.stdout.flush()
    return result


def command(argv, env=None):
    result = command_result(argv, env)
    if result.returncode: raise ValueError("Builder or test process failed")
    return result.stdout


def mac_required():
    if sys.platform != "darwin" or platform.machine().lower() != "arm64": raise ValueError("Requires the macOS arm64 adapter")


def output_reference(path, aliases):
    path = Path(path).resolve()
    for alias in aliases:
        if path.is_relative_to(alias.resolve()):
            return (alias.relative_to(ROOT) / path.relative_to(alias.resolve())).as_posix()
    raise ValueError("Builder returned an artifact outside its declared output directories")


def build_runtime(args):
    mac_required()
    cargo = [args.cargo_executable] if args.cargo_executable else ["cargo", "+1.99.1"]
    env = dict(os.environ); env.pop("CARGO_BUILD_TARGET", None)
    target = ROOT / "target"
    output = command(cargo + ["build", "-p", "latexide-ffi", "-p", "latexide-mcp", "--locked", "--target-dir", str(target), "--message-format=json"] + (["--offline"] if args.offline else []), env=env)
    expected = {"ffi_static": target / "debug/liblatexide_ffi.a", "ffi_dynamic": target / "debug/liblatexide_ffi.dylib",
                "mcp_executable": target / "debug/latexide-mcp"}
    emitted = set()
    for line in output.splitlines():
        try: message = json.loads(line)
        except ValueError: continue
        if message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") in {"latexide_ffi", "latexide-mcp"}:
            emitted.update(Path(path).resolve() for path in message.get("filenames", []))
            if message.get("executable"): emitted.add(Path(message["executable"]).resolve())
    if not all(path.resolve() in emitted and path.is_file() for path in expected.values()): raise ValueError("Cargo did not emit every required runtime artifact")
    write_json(args.result, {"schema": 1, "artifacts": [{"path": output_reference(path, [target]), "kind": kind} for kind, path in expected.items()]})


def swift_options():
    return ["--package-path", "apps/macos", "--scratch-path", str(ROOT / ".local/p00-native-build"), "--disable-sandbox", "--build-system", "native"]


def build_native(args):
    mac_required(); options = swift_options()
    # Rebuild the test image after the external Rust static library changes.
    command(["swift", "package", *options, "clean"])
    command(["swift", "build", *options, "--build-tests"])
    result = subprocess.run(["swift", "build", *options, "--show-bin-path"], cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8")
    directory = Path(result.stdout.strip())
    binary = directory / "LaTeXIDEPackageTests.xctest/Contents/MacOS/LaTeXIDEPackageTests"
    # Bundle.module reads this copied resource at runtime, outside the image.
    artifacts = {"native_test_binary": binary,
                 "native_theme_resource": directory / "IDEThemeAssets_IDEThemeAssets.bundle/editor-themes.json"}
    if not all(path.is_file() for path in artifacts.values()): raise ValueError("SwiftPM did not emit every required test artifact")
    write_json(args.result, {"schema": 1, "artifacts": [
        {"path": output_reference(path, [ROOT / ".local/p00-native-build"]), "kind": kind} for kind, path in artifacts.items()]})


def main():
    parser = argparse.ArgumentParser(); subs = parser.add_subparsers(dest="command", required=True)
    tests = subs.add_parser("unittest"); tests.add_argument("--suite", choices=["coordinator", "ipc", "mcp", "tools"], required=True)
    tests.add_argument("--result", type=Path, required=True)
    runtime = subs.add_parser("runtime-build"); runtime.add_argument("--cargo-executable"); runtime.add_argument("--offline", action="store_true"); runtime.add_argument("--result", type=Path, required=True)
    for name in ["native-build", "native-controls", "docs"]:
        sub = subs.add_parser(name); sub.add_argument("--result", type=Path, required=True)
    schema = subs.add_parser("native-schema"); schema.add_argument("--binary", type=Path, required=True); schema.add_argument("--result", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "unittest":
            if args.suite in {"ipc", "mcp"}: mac_required()
            sys.path.insert(0, str(ROOT / "tests"))
            if args.suite in {"coordinator", "tools"}:
                suite = unittest.defaultTestLoader.discover(str(ROOT / ("tests/contracts" if args.suite == "coordinator" else "tests")), pattern="test_*.py")
            else:
                module = importlib.import_module("ipc_contract" if args.suite == "ipc" else "mcp_contract")
                suite = unittest.defaultTestLoader.loadTestsFromModule(module)
            return run_unittest(suite, args.suite, args.result)
        if args.command == "runtime-build": build_runtime(args)
        elif args.command == "native-build": build_native(args)
        elif args.command == "native-controls":
            mac_required(); env = dict(os.environ)
            for name in ["LATEXIDE_CODEX_HANDSHAKE", "LATEXIDE_RENDER_PREVIEWS"]: env.pop(name, None)
            result = command_result(["swift", "test", *swift_options(), "--skip-build"], env=env)
            summary = parse_native_summary(result.stdout); write_json(args.result, summary)
            return 0 if result.returncode == 0 and summary["failures"] == 0 else 1
        elif args.command == "native-schema":
            mac_required(); command([str(args.binary), str(ROOT / "tests/contracts/v1")])
            write_json(args.result, {"schema": 1, "suite": "native-schema", "tests": 1, "passed": 1, "skipped": 0, "failures": 0, "errors": 0,
                                     "skipped_ids": [], "scope": "one program checking five wire-shape families; no UI/provider acceptance"})
        elif args.command == "docs": write_json(args.result, check_documents(ROOT, DOCUMENTS))
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "failed", "reason": str(error)})); return 1


if __name__ == "__main__": raise SystemExit(main())
