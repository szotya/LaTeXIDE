"""Provider checks must use current binaries and retain their actual test scope."""
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from contextlib import redirect_stdout

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("p00_integrated_verify", ROOT / "integration/registry/verify.py")
verify = importlib.util.module_from_spec(spec); spec.loader.exec_module(verify)


class IntegratedVerificationContract(unittest.TestCase):
    def script(self, body, *args): return [sys.executable, "-c", body, *args]

    def probes(self):
        path = ROOT / "integration/registry/probes.py"
        self.assertTrue(path.is_file(), "The real provider adapter is missing")
        spec = importlib.util.spec_from_file_location("p00_probe_contract", path)
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module

    def native_fixture(self, root):
        directory = root / ".local/p00-native-build/debug"
        binary = directory / "LaTeXIDEPackageTests.xctest/Contents/MacOS/LaTeXIDEPackageTests"
        resource = directory / "IDEThemeAssets_IDEThemeAssets.bundle/editor-themes.json"
        for path in [binary, resource]:
            path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b"current fixture")
        return directory, resource

    def test_native_build_binds_the_generated_resource_beside_the_test_binary(self):
        module = self.probes()
        with tempfile.TemporaryDirectory() as d:
            root = Path(d).resolve(); directory, resource = self.native_fixture(root)
            with patch.object(module, "ROOT", root), patch.object(module, "mac_required"), patch.object(module, "command"), \
                 patch.object(module.subprocess, "run", return_value=SimpleNamespace(stdout=str(directory))), \
                 patch.dict(module.os.environ, {"P00_VERIFICATION_RUN_ID": "fixture"}):
                module.build_native(SimpleNamespace(result=root / "index.json"))
            artifacts = verify.capture_artifacts(root, {"artifact_index": "index.json"}, "fixture", [root])
            self.assertTrue(verify.artifacts_current(root, artifacts, [root]))
            resource.write_bytes(b"replacement palette")
            self.assertFalse(verify.artifacts_current(root, artifacts, [root]), "A copied resource replacement must invalidate native evidence")
            self.assertEqual({a["kind"] for a in artifacts}, {"native_test_binary", "native_theme_resource"})

    def test_native_resource_is_mandatory_in_the_builder_and_profile(self):
        module = self.probes()
        args = SimpleNamespace(profile="integrated", cargo_executable=None, offline=True, native_schema=False, run_dir=ROOT / ".local/fixture-run")
        step = next(s for s in verify.build_steps(ROOT, args, "darwin", "arm64") if s["name"] == "native-build")
        with self.subTest(boundary="profile"):
            self.assertIn("native_theme_resource", step["required_artifact_kinds"])
        with self.subTest(boundary="builder"), tempfile.TemporaryDirectory() as d:
            root = Path(d).resolve(); directory, resource = self.native_fixture(root); resource.unlink()
            with patch.object(module, "ROOT", root), patch.object(module, "mac_required"), patch.object(module, "command"), \
                 patch.object(module.subprocess, "run", return_value=SimpleNamespace(stdout=str(directory))):
                with self.assertRaises(ValueError): module.build_native(SimpleNamespace(result=root / "index.json"))

    def test_every_declared_binary_is_bound_and_one_swapped_file_blocks_use(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source").write_bytes(b"fixture")
            steps = [
                {"name": "build", "kind": "build", "artifact_paths": [{"path": "ffi", "kind": "ffi_dynamic"}, {"path": "bridge", "kind": "mcp_executable"}],
                 "argv": self.script("from pathlib import Path;Path('ffi').write_bytes(b'ffi');Path('bridge').write_bytes(b'bridge')")},
                {"name": "replace", "kind": "fixture", "argv": self.script("from pathlib import Path;Path('bridge').write_bytes(b'old bridge')")},
                {"name": "use", "kind": "mcp", "depends_on": ["build"], "consumes_artifacts": ["build"], "argv": self.script("print('PASS')")},
            ]
            report = verify.run_checks(root, ["source"], steps, root / "logs", 3)
            self.assertEqual(len(report["checks"][0].get("artifacts", [])), 2)
            self.assertEqual(report["checks"][2]["status"], "stale_artifact")
            self.assertNotEqual(report["status"], "passed")

    def test_replacement_during_a_consumer_cannot_be_current_evidence(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source").write_bytes(b"fixture")
            steps = [
                {"name": "build", "kind": "build", "artifact_path": "native", "argv": self.script("from pathlib import Path;Path('native').write_bytes(b'new')")},
                {"name": "use", "kind": "native", "depends_on": ["build"], "consumes_artifact": {"producer": "build", "path": "native"},
                 "argv": self.script("from pathlib import Path;Path('native').write_bytes(b'replaced')")},
            ]
            report = verify.run_checks(root, ["source"], steps, root / "logs", 3)
            self.assertEqual(report["checks"][1]["status"], "stale_artifact")

    def test_zero_exit_cannot_hide_a_missing_summary_or_unapproved_skip(self):
        for summary, wanted in [(None, "failed"),
                        ({"schema": 1, "suite": "ipc", "tests": 0, "passed": 0, "skipped": 0, "failures": 0, "errors": 0, "skipped_ids": []}, "failed"),
                        ({"schema": 1, "suite": "ipc", "tests": 2, "passed": 1, "skipped": 1, "failures": 0, "errors": 0, "skipped_ids": ["required_test"]}, "failed"),
                        ({"schema": 1, "suite": "ipc", "tests": 2, "passed": 2, "skipped": 0, "failures": 0, "errors": 0, "skipped_ids": []}, "passed")]:
            with self.subTest(summary=summary), tempfile.TemporaryDirectory() as d:
                root = Path(d); (root / "source").write_bytes(b"fixture")
                argv = self.script("pass") if summary is None else self.script("import os,json,sys;from pathlib import Path;d=json.loads(sys.argv[1]);d['verification_run_id']=os.environ['P00_VERIFICATION_RUN_ID'];Path('summary.json').write_text(json.dumps(d))", json.dumps(summary))
                steps = [{"name": "ipc", "kind": "ipc", "argv": argv, "summary_path": "summary.json", "summary_suite": "ipc"}]
                report = verify.run_checks(root, ["source"], steps, root / "logs", 3)
                self.assertEqual(report["status"], wanted)

    def test_explicit_output_cache_is_allowed_without_authorizing_arbitrary_files(self):
        with tempfile.TemporaryDirectory() as d, tempfile.TemporaryDirectory() as c, tempfile.TemporaryDirectory() as other:
            root = Path(d); cache = Path(c); (root / "source").write_bytes(b"fixture")
            path = cache / "native"
            steps = [{"name": "build", "kind": "build", "artifact_path": str(path),
                      "argv": self.script("import sys;from pathlib import Path;Path(sys.argv[1]).write_bytes(b'new')", str(path))}]
            self.assertEqual(verify.run_checks(root, ["source"], steps, root / "logs", 3)["status"], "failed")
            self.assertIn("artifact_roots", verify.run_checks.__code__.co_varnames)
            report = verify.run_checks(root, ["source"], steps, root / "logs", 3, artifact_roots=[cache])
            self.assertEqual(report["status"], "passed")
            steps[0]["artifact_path"] = str(Path(other) / "native")
            self.assertEqual(verify.run_checks(root, ["source"], steps, root / "logs", 3, artifact_roots=[cache])["status"], "failed")

    def test_skip_identity_count_must_match_even_for_an_allowed_optional_test(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source").write_bytes(b"fixture")
            data = {"schema": 1, "suite": "fixture", "tests": 2, "passed": 1, "skipped": 1, "failures": 0, "errors": 0, "skipped_ids": ["optional", "optional"]}
            command = self.script("import os,json,sys;from pathlib import Path;d=json.loads(sys.argv[1]);d['verification_run_id']=os.environ['P00_VERIFICATION_RUN_ID'];Path('summary.json').write_text(json.dumps(d))", json.dumps(data))
            step = {"name": "test", "kind": "fixture", "argv": command, "summary_path": "summary.json", "summary_suite": "fixture", "allowed_skips": ["optional"]}
            self.assertEqual(verify.run_checks(root, ["source"], [step], root / "logs", 3)["status"], "failed")

    def test_failed_command_retains_its_current_machine_test_failure_counts(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source").write_bytes(b"fixture")
            data = {"schema": 1, "suite": "fixture", "tests": 1, "passed": 0, "skipped": 0, "failures": 1, "errors": 0, "skipped_ids": []}
            command = self.script("import os,json,sys;from pathlib import Path;d=json.loads(sys.argv[1]);d['verification_run_id']=os.environ['P00_VERIFICATION_RUN_ID'];Path('summary.json').write_text(json.dumps(d));raise SystemExit(1)", json.dumps(data))
            step = {"name": "test", "kind": "fixture", "argv": command, "summary_path": "summary.json", "summary_suite": "fixture"}
            report = verify.run_checks(root, ["source"], [step], root / "logs", 3)
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["checks"][0].get("test_summary", {}).get("failures"), 1)

    def test_actual_native_test_and_guide_inputs_change_the_fingerprint(self):
        inputs = ["tests/ipc_contract.py", "tests/mcp_contract.py", "apps/macos/Package.swift",
                  "apps/macos/Sources/LaTeXIDEKit/NativeRuntime.swift", "apps/macos/CLatexCore/module.modulemap",
                  "shared/ide-theme/Sources/IDEThemeAssets/Palette.json", "include/latexide_core.h",
                  ".github/workflows/ci.yml", "docs/development/contracts/v1.md",
                  "scripts/acquire_tools.py", "tools/toolchain-lock.json",
                  "skills/latexide-platform-development/references/parallel-development.md"]
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", *inputs]:
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b"before")
            for name in inputs:
                with self.subTest(path=name):
                    before = verify.source_identity(root, lambda: verify.discover_sources(root))["sha256"]
                    (root / name).write_bytes(b"after")
                    after = verify.source_identity(root, lambda: verify.discover_sources(root))["sha256"]
                    self.assertNotEqual(before, after)
                    (root / name).write_bytes(b"before")

    def test_integrated_profile_requires_real_providers_and_marks_other_hosts_unrun(self):
        self.assertTrue(callable(getattr(verify, "build_steps", None)), "Integrated provider profile is missing")
        args = SimpleNamespace(profile="integrated", cargo_executable=None, offline=True, native_schema=False, run_dir=ROOT / ".local/fixture-run")
        mac = {s["name"]: s for s in verify.build_steps(ROOT, args, "darwin", "arm64")}
        for name in ["runtime-build", "ipc-roundtrip", "mcp-roundtrip", "native-build", "native-controls"]:
            self.assertTrue(mac[name]["argv"])
        self.assertIn("runtime-build", mac["mcp-roundtrip"]["consumes_artifacts"])
        for platform, machine in [("win32", "AMD64"), ("linux", "x86_64"), ("darwin", "x86_64")]:
            steps = {s["name"]: s for s in verify.build_steps(ROOT, args, platform, machine)}
            for name in ["ipc-roundtrip", "mcp-roundtrip", "native-controls"]:
                self.assertIsNone(steps[name]["argv"])
                self.assertTrue(steps[name]["required"])

    def test_probe_records_real_unittest_counts_and_rejects_unknown_suites(self):
        module = self.probes()
        class Samples(unittest.TestCase):
            def test_ok(self): self.assertEqual(1 + 1, 2)
            @unittest.skip("explicit optional fixture")
            def test_optional(self): pass
        with tempfile.TemporaryDirectory() as d:
            output = Path(d) / "summary.json"
            code = module.run_unittest(unittest.defaultTestLoader.loadTestsFromTestCase(Samples), "fixture", output, stream=io.StringIO())
            self.assertEqual(code, 0)
            data = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual((data["tests"], data["passed"], data["skipped"], data["failures"], data["errors"]), (2, 1, 1, 0, 0))
            bad = subprocess.run([sys.executable, str(ROOT / "integration/registry/probes.py"), "unittest", "--suite", "arbitrary-command", "--result", str(output)], capture_output=True, timeout=10)
            self.assertNotEqual(bad.returncode, 0)

    def test_native_summary_needs_the_all_tests_scope_and_preserves_optional_skips(self):
        module = self.probes()
        text = "Test Suite 'All tests' passed at 2026-10-31.\nExecuted 73 tests, with 4 tests skipped and 0 failures (0 unexpected) in 1 seconds\n"
        names = ["AssistantSessionTests.testInstalledCodexInitializeOnly", "IntelligenceRenderTests.testRenderAssistantAndCompletionControls",
                 "ProjectCompletionRenderTests.testRenderProjectMacroReferenceAndCitationCandidates", "WorkspaceRenderTests.testRenderPaletteWorkspaces"]
        skips = "\n".join(f"x.swift:1: -[LaTeXIDEKitTests.{name.split('.')[0]} {name.split('.')[1]}] : Test skipped - opt-in" for name in names)
        summary = module.parse_native_summary(skips + "\n" + text)
        self.assertEqual((summary["tests"], summary["passed"], summary["skipped"]), (73, 69, 4))
        self.assertEqual(set(summary["skipped_ids"]), set(names))
        with self.assertRaises(ValueError): module.parse_native_summary("Executed 5 tests, with 0 failures in 1 seconds")

    def test_unittest_assertion_events_are_not_subtracted_as_test_cases(self):
        module = self.probes()
        class Failing(unittest.TestCase):
            def test_subcases(self):
                for number in [1, 2]:
                    with self.subTest(number=number): self.assertEqual(number, 0)
        with tempfile.TemporaryDirectory() as d:
            result = Path(d) / "summary.json"
            self.assertEqual(module.run_unittest(unittest.defaultTestLoader.loadTestsFromTestCase(Failing), "fixture", result, stream=io.StringIO()), 1)
            data = json.loads(result.read_text(encoding="utf-8"))
            self.assertEqual((data["tests"], data["passed"], data.get("failed_tests"), data["failures"]), (1, 0, 1, 2))

    def test_xctest_multiple_assertions_in_one_failed_test_keep_valid_counts(self):
        module = self.probes()
        text = "Test Case '-[LaTeXIDEKitTests.Example testCase]' failed (0.1 seconds).\nTest Suite 'All tests' failed at 2026-10-31.\nExecuted 1 test, with 2 failures (0 unexpected) in 1 seconds\n"
        try: data = module.parse_native_summary(text)
        except ValueError: data = {}
        self.assertEqual((data.get("tests"), data.get("passed"), data.get("failed_tests"), data.get("failures")), (1, 0, 1, 2))

    def test_documentation_probe_rejects_broken_local_links(self):
        module = self.probes()
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "guide.md").write_text("[missing](missing.md)", encoding="utf-8")
            with self.assertRaises(ValueError): module.check_documents(root, ["guide.md"])
            (root / "missing.md").write_text("fixture", encoding="utf-8")
            data = module.check_documents(root, ["guide.md"])
            self.assertEqual(data["checked_links"], 1)
            self.assertEqual(len(data["files"][0]["sha256"]), 64)

    def test_current_code_checks_do_not_close_partial_requirement_acceptance(self):
        self.assertTrue(callable(getattr(verify, "requirement_inventory", None)), "Current evidence is not joined to the requirement inventory")
        report = {"status": "passed", "source_sha256": "a" * 64, "checks": []}
        inventory = verify.requirement_inventory(ROOT, report)
        self.assertEqual(inventory["LANG-06"]["baseline"]["observed_status"], "partial")
        self.assertEqual(inventory["LANG-06"]["acceptance"], "partial")
        self.assertIn("MCP-03", inventory["QA-08"]["unqualified_prerequisites"])
        self.assertNotEqual(inventory["QA-08"]["acceptance"], "implemented")

    def test_inventory_without_any_bound_current_check_is_unrun(self):
        inventory = verify.requirement_inventory(ROOT, {"status": "passed", "source_sha256": "a" * 64, "checks": []})
        self.assertEqual(inventory["LANG-06"]["baseline"]["observed_status"], "partial")
        self.assertEqual(inventory["LANG-06"]["current_verification"]["status"], "unrun")

    def test_nonfinite_timeout_does_not_launch_a_child(self):
        with tempfile.TemporaryDirectory() as d:
            for timeout in [float("nan"), float("inf")]:
                with self.subTest(timeout=timeout), self.assertRaises(ValueError):
                    verify.execute_step("invalid", self.script("pass"), Path(d), Path(d) / "log", timeout)

    def test_invalid_inventory_does_not_leave_a_previous_green_report(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]: (root / name).write_bytes(b"fixture")
            audit = root / "docs/ide-audit-2026-10-31/backlog.json"; audit.parent.mkdir(parents=True); audit.write_bytes(b"{")
            output = root / "report.json"; output.write_text('{"status":"passed"}', encoding="utf-8")
            code = None
            with patch.object(verify, "ROOT", root), patch.object(sys, "argv", ["verify", "--profile", "structure", "--output", str(output)]), redirect_stdout(io.StringIO()):
                try: code = verify.main()
                except (ValueError, OSError, KeyError): pass
            self.assertEqual(code, 1, "Invalid current inputs must still produce a failed current report")
            self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["status"], "failed")


if __name__ == "__main__": unittest.main()
