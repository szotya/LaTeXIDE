"""Stored verification evidence must stay bound to the current checkout."""
import copy
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("p00_verify_recheck", ROOT / "integration/registry/verify.py")
verify = importlib.util.module_from_spec(spec); spec.loader.exec_module(verify)


class EvidenceContract(unittest.TestCase):
    def evidence(self):
        spec = importlib.util.spec_from_file_location("p00_evidence_test", ROOT / "integration/registry/evidence.py")
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module

    def source(self, root):
        return verify.source_identity(root, lambda: verify.discover_sources(root))

    def fixture(self, root):
        for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crates/source.rs", "docs/development/contracts/guide.md"]:
            path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text("fixture", encoding="utf-8")
        for name in ["docs/ide-audit-2026-10-31/backlog.json", "docs/development/package-specs.json",
                     "integration/registry/owners.v1.json", "integration/registry/ports.v1.json", "integration/registry/p00-acceptance.v1.json"]:
            path = root / name; path.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(ROOT / name, path)
        def command(code): return [sys.executable, "-c", code]
        program = (
            "import importlib.util,unittest;from pathlib import Path;"
            f"s=importlib.util.spec_from_file_location('probe',{str(ROOT / 'integration/registry/probes.py')!r});"
            "p=importlib.util.module_from_spec(s);s.loader.exec_module(p);"
            "C=type('Example',(unittest.TestCase,),{'test_ok':lambda self:self.assertEqual(2+2,4)});"
            "raise SystemExit(p.run_unittest(unittest.defaultTestLoader.loadTestsFromTestCase(C),'fixture',Path('summary.json')))"
        )
        steps = [
            {"name": "build", "kind": "fixture-build", "artifact_paths": [{"path": "image", "kind": "binary"}, {"path": "palette", "kind": "resource"}],
             "argv": command("from pathlib import Path;Path('image').write_bytes(b'image');Path('palette').write_bytes(b'palette')")},
            {"name": "unit", "kind": "python-unit", "depends_on": ["build"], "consumes_artifacts": ["build"],
             "argv": command(program), "summary_path": "summary.json", "summary_suite": "fixture"},
            {"name": "optional", "kind": "unselected", "argv": None, "required": False},
        ]
        report = verify.run_checks(root, lambda: verify.discover_sources(root), steps, root / ".local/logs", 5)
        self.assertEqual(report["status"], "passed")
        report.update(profile="structure", selection={"native_schema": False, "platform": sys.platform, "machine": verify.host_platform.machine()},
                      run_directory=".local/logs", verification_id="a" * 32, description_identity=[])
        return steps, report

    def check(self, root, report, steps):
        return self.evidence().revalidate_report(root, report, self.source(root), steps)

    def test_fresh_report_retains_its_actual_optional_unrun_scope(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            result = self.check(root, report, steps)
            self.assertEqual(result["status"], "current")
            self.assertEqual(report["checks"][-1]["status"], "unrun")

    def test_added_or_changed_source_invalidates_a_stored_success(self):
        for name in ["crates/source.rs", "crates/new.rs", "docs/development/contracts/guide.md"]:
            with self.subTest(path=name), tempfile.TemporaryDirectory() as d:
                root = Path(d); steps, report = self.fixture(root); (root / name).write_text("changed", encoding="utf-8")
                self.assertEqual(self.check(root, report, steps)["status"], "stale_source")

    def test_resource_and_log_replacement_invalidate_evidence(self):
        for resource in [True, False]:
            with self.subTest(resource=resource), tempfile.TemporaryDirectory() as d:
                root = Path(d); steps, report = self.fixture(root)
                path = root / "palette" if resource else Path(report["checks"][1]["log"]["path"])
                path.write_bytes(b"replaced")
                result = self.check(root, report, steps)
                self.assertEqual(result["status"], "stale_artifact" if resource else "stale_log")

    def test_missing_duplicate_or_weakened_check_cannot_pass(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            variants = []
            missing = copy.deepcopy(report); missing["checks"].pop(1); variants.append(missing)
            duplicate = copy.deepcopy(report); duplicate["checks"].append(copy.deepcopy(duplicate["checks"][0])); variants.append(duplicate)
            weakened = copy.deepcopy(report); weakened["checks"][1]["required"] = False; variants.append(weakened)
            unrun = copy.deepcopy(report); unrun["checks"][1]["status"] = "unrun"; variants.append(unrun)
            fake_optional = copy.deepcopy(report); fake_optional["checks"][-1]["status"] = "passed"; variants.append(fake_optional)
            for data in variants:
                with self.subTest(data=data["checks"]): self.assertNotEqual(self.check(root, data, steps)["status"], "current")

    def test_missing_resource_or_changed_consumption_is_invalid(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            missing = copy.deepcopy(report); missing["checks"][0]["artifacts"].pop()
            swapped = copy.deepcopy(report); swapped["checks"][1]["consumed_artifacts"][0]["sha256"] = "0" * 64
            for data in [missing, swapped]:
                with self.subTest(data=data): self.assertNotEqual(self.check(root, data, steps)["status"], "current")

    def test_invalid_test_counts_or_skip_cannot_pass(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            for update in [{"tests": 0}, {"skipped": 1, "skipped_tests": 1, "passed": 0, "skipped_ids": ["unapproved"]}]:
                data = copy.deepcopy(report); data["checks"][1]["test_summary"].update(update)
                self.assertNotEqual(self.check(root, data, steps)["status"], "current")

    def test_boolean_exit_code_or_sequence_is_not_integer_evidence(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            for key, value in [("exit_code", False), ("sequence", True)]:
                data = copy.deepcopy(report); data["checks"][0][key] = value
                with self.subTest(key=key): self.assertNotEqual(self.check(root, data, steps)["status"], "current")

    def test_resource_changed_during_summary_inspection_is_not_current(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root); module = self.evidence(); original = module.read_json
            def late_change(path, *args):
                value = original(path, *args); (root / "palette").write_bytes(b"late replacement"); return value
            with patch.object(module, "read_json", side_effect=late_change):
                result = module.revalidate_report(root, report, self.source(root), steps)
            self.assertEqual(result["status"], "stale_artifact")

    def test_failed_report_and_false_description_cannot_pass(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            failed = copy.deepcopy(report); failed["status"] = "failed"
            self.assertEqual(self.check(root, failed, steps)["status"], "failed_report")
            report["description_identity"] = [{"files": [{"path": "not-a-guide", "sha256": "0" * 64}]}]
            self.assertNotEqual(self.check(root, report, steps)["status"], "current")

    def test_cli_recheck_never_executes_report_commands_or_rewrites_input(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            sentinel = root / "must-not-exist"
            report["checks"][0]["command"] = [sys.executable, "-c", f"from pathlib import Path;Path({str(sentinel)!r}).touch()"]
            path = root / "report.json"; original = json.dumps(report).encode(); path.write_bytes(original)
            output = subprocess.run([sys.executable, str(ROOT / "integration/registry/verify.py"), "--recheck-report", str(path)], capture_output=True, timeout=15)
            self.assertNotEqual(output.returncode, 0)
            self.assertEqual(json.loads(output.stdout)["status"], "stale_source")
            self.assertFalse(sentinel.exists()); self.assertEqual(path.read_bytes(), original)

    def test_current_cli_recheck_uses_trusted_steps_and_writes_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = self.fixture(root)
            path = root / "report.json"; original = json.dumps(report).encode(); path.write_bytes(original)
            with patch.object(verify, "ROOT", root), patch.object(verify, "build_steps", return_value=steps), \
                 patch.object(sys, "argv", ["verify.py", "--recheck-report", str(path)]), \
                 patch.object(verify, "run_checks", side_effect=AssertionError("Recheck must not execute")), patch("sys.stdout", new_callable=io.StringIO):
                self.assertEqual(verify.main(), 0)
            self.assertEqual(path.read_bytes(), original)
