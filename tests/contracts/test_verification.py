import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("p00_verify", ROOT / "integration/registry/verify.py")
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


class VerificationContract(unittest.TestCase):
    def test_exit_codes_and_log_artifact_hashes_are_real(self):
        with tempfile.TemporaryDirectory(prefix="p00 fixture ") as d:
            root = Path(d)
            good = verify.execute_step("good", [sys.executable, "-c", "print('fixture')"], root, root / "good.log", 3)
            bad = verify.execute_step("bad", [sys.executable, "-c", "print('PASS');raise SystemExit(3)"], root, root / "bad.log", 3)
            self.assertEqual((good["status"], good["exit_code"]), ("passed", 0))
            self.assertEqual((bad["status"], bad["exit_code"]), ("failed", 3))
            self.assertEqual(len(good["log"]["sha256"]), 64)
            self.assertEqual(good["log"]["kind"], "command_log")
            self.assertIsNone(good["artifact_sha256"])
            self.assertNotEqual(good["log"]["sha256"], bad["log"]["sha256"])

    def test_timeout_is_never_pass_even_if_output_says_pass(self):
        with tempfile.TemporaryDirectory() as d:
            result = verify.execute_step("slow", [sys.executable, "-u", "-c", "import time;print('PASS');time.sleep(10)"], Path(d), Path(d) / "slow.log", .1)
            self.assertEqual(result["status"], "timed_out")
            self.assertTrue(result["timed_out"])

    def test_source_changes_during_a_check_invalidate_the_whole_report(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source.txt").write_text("before")
            steps = [{"name": "mutator", "argv": [sys.executable, "-c", "from pathlib import Path;Path('source.txt').write_text('after')"], "kind": "unit", "required": True}]
            result = verify.run_checks(root, ["source.txt"], steps, root / "logs", 3)
            self.assertEqual(result["status"], "source_changed")
            self.assertNotEqual(result["source_sha256"], result["source_sha256_after"])

    def test_generator_input_changes_are_included_in_source_identity(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]: (root / name).write_text("fixture")
            spec = root / "docs/development/package-specs.json"; spec.parent.mkdir(parents=True); spec.write_text("{}")
            steps = [{"name": "change-spec", "argv": [sys.executable, "-c", "from pathlib import Path;Path('docs/development/package-specs.json').write_text('{\"changed\":true}')"], "kind": "source"}]
            report = verify.run_checks(root, lambda: verify.discover_sources(root), steps, root / "logs", 3)
            self.assertEqual(report["status"], "source_changed")

    def test_failed_build_cannot_run_a_previous_binary_as_current_evidence(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source.txt").write_text("fixture")
            steps = [
                {"name": "build", "argv": [sys.executable, "-c", "raise SystemExit(1)"], "kind": "build"},
                {"name": "run", "argv": [sys.executable, "-c", "print('old binary PASS')"], "kind": "native-schema", "depends_on": ["build"]},
            ]
            report = verify.run_checks(root, ["source.txt"], steps, root / "logs", 3)
            self.assertEqual(report["checks"][1]["status"], "unrun")
            self.assertIsNone(report["checks"][1]["exit_code"])

    def test_artifact_replacement_between_build_and_run_is_stale(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / "source.txt").write_text("fixture")
            steps = [
                {"name": "build", "argv": [sys.executable, "-c", "from pathlib import Path;Path('native').write_text('original')"], "kind": "build", "artifact_path": "native"},
                {"name": "replace", "argv": [sys.executable, "-c", "from pathlib import Path;Path('native').write_text('changed')"], "kind": "source"},
                {"name": "run", "argv": [sys.executable, "-c", "print('PASS')"], "kind": "native-schema", "depends_on": ["build"], "consumes_artifact": {"producer": "build", "path": "native"}},
            ]
            report = verify.run_checks(root, ["source.txt"], steps, root / "logs", 3)
            self.assertEqual(report["checks"][2]["status"], "stale_artifact")

    def test_review_focus_r3_newer_partial_observation_survives_old_missing_audit(self):
        old = {"platform": "macos", "kind": "capability", "capability_id": "LANG-06", "source_sha256": "source", "sequence": 1, "status": "missing"}
        new = dict(old, sequence=2, status="partial")
        for rows in [[old, new], [new, old]]:
            self.assertEqual(verify.resolve_evidence(rows, "source", "macos", "capability", capability_id="LANG-06"), "partial")

    def test_review_focus_r4_platform_kind_and_unrun_are_not_promoted(self):
        rust = {"platform": "windows-server", "kind": "rust-unit", "source_sha256": "source", "sequence": 1, "status": "passed", "exit_code": 0}
        self.assertEqual(verify.resolve_evidence([rust], "source", "windows11", "live-ui"), "unrun")
        skipped = dict(rust, status="skipped", exit_code=None)
        self.assertEqual(verify.resolve_evidence([skipped], "source", "windows-server", "rust-unit"), "skipped")
        self.assertEqual(verify.resolve_evidence([rust], "new-source", "windows-server", "rust-unit"), "stale_source")
        invalid = dict(rust, exit_code=1)
        self.assertEqual(verify.resolve_evidence([invalid], "source", "windows-server", "rust-unit"), "failed")
        self.assertEqual(verify.resolve_evidence([rust], "source", "windows-server", "rust-unit", artifact_sha256="expected"), "stale_artifact")

    def test_review_focus_r5_generation_is_byte_deterministic(self):
        spec = importlib.util.spec_from_file_location("p00_generate", ROOT / "integration/registry/generate_contracts.py")
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        self.assertEqual(module.render(), module.render())


if __name__ == "__main__": unittest.main()
