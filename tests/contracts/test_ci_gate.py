"""The required GitHub check must not turn missing/failed work into success."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "integration/registry/ci_gate.py"


class CIGateContract(unittest.TestCase):
    def call(self, *args, needs=None, output=None):
        env = dict(os.environ)
        env.pop("GITHUB_OUTPUT", None)
        if needs is not None: env["P00_CI_NEEDS"] = json.dumps(needs)
        if output is not None: env["GITHUB_OUTPUT"] = str(output)
        return subprocess.run([sys.executable, str(GATE), *args], env=env, capture_output=True, text=True, encoding="utf-8", timeout=30)

    def git(self, root, *args):
        env = dict(os.environ, GIT_AUTHOR_NAME="P00 fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                   GIT_COMMITTER_NAME="P00 fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid")
        return subprocess.check_output(["git", "-C", str(root), *args], env=env, stderr=subprocess.DEVNULL, text=True).strip()

    def commit(self, root):
        self.git(root, "add", "--all"); self.git(root, "commit", "--quiet", "-m", "fixture")
        return self.git(root, "rev-parse", "HEAD")

    def test_docs_only_diff_outputs_skip_but_source_changes_require_runtime(self):
        with tempfile.TemporaryDirectory(prefix="p00 CI ") as d:
            root = Path(d); self.git(root, "init", "--quiet")
            (root / "README.md").write_bytes(b"initial"); base = self.commit(root)
            cases = [("docs/guide.md", False), ("skills/example/SKILL.md", False),
                     ("crates/core/src/new.rs", True), ("apps/macos/Feature.swift", True),
                     ("integration/requests/P07.json", True), (".github/workflows/ci.yml", True),
                     ("docs/development/package-specs.json", True), ("unknown/file.md", True)]
            for index, (name, wanted) in enumerate(cases):
                with self.subTest(path=name):
                    path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b"fixture")
                    head = self.commit(root); output = root / ".git" / f"output-{index}"
                    result = self.call("changes", "--root", str(root), "--base", base, "--head", head, output=output)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(result.stdout)["runtime_required"], wanted)
                    self.assertEqual(output.read_text(encoding="utf-8"), "runtime=" + str(wanted).lower() + "\n")
                    base = head

    def test_code_renamed_to_docs_does_not_hide_the_deleted_code_path(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.git(root, "init", "--quiet"); (root / "crates").mkdir()
            (root / "crates/code.rs").write_bytes(b"same content\n"); base = self.commit(root)
            (root / "docs").mkdir(); (root / "crates/code.rs").rename(root / "docs/code.md")
            head = self.commit(root)
            result = self.call("changes", "--root", str(root), "--base", base, "--head", head)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(json.loads(result.stdout)["runtime_required"])

    def test_missing_diff_base_cannot_be_assumed_to_be_docs_only(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.git(root, "init", "--quiet"); (root / "README.md").write_bytes(b"fixture")
            head = self.commit(root)
            result = self.call("changes", "--root", str(root), "--base", "a" * 40, "--head", head)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('"failed"', result.stdout)
            initial = self.call("changes", "--root", str(root), "--base", "0" * 40, "--head", head)
            self.assertEqual(initial.returncode, 0, initial.stderr)
            self.assertTrue(json.loads(initial.stdout)["runtime_required"])

    def test_docs_only_and_completed_runtime_have_a_successful_required_gate(self):
        for required, runtime in [("false", "skipped"), ("true", "success")]:
            with self.subTest(required=required):
                needs = {"planning": {"result": "success"},
                         "changes": {"result": "success", "outputs": {"runtime": required}},
                         "runtime": {"result": runtime}}
                result = self.call("aggregate", needs=needs)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(json.loads(result.stdout)["status"], "passed")

    def test_required_work_failure_skip_cancel_or_missing_metadata_cannot_pass(self):
        for job in ["planning", "changes", "runtime"]:
            for status in ["failure", "cancelled", "skipped", "timed_out", ""]:
                with self.subTest(job=job, status=status):
                    needs = {"planning": {"result": "success"},
                             "changes": {"result": "success", "outputs": {"runtime": "true"}},
                             "runtime": {"result": "success"}}
                    needs[job]["result"] = status
                    result = self.call("aggregate", needs=needs)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('"failed"', result.stdout)
        for needs in [{}, {"planning": {"result": "success"}},
                      {"planning": {"result": "success"}, "changes": {"result": "success"}, "runtime": {"result": "skipped"}}]:
            result = self.call("aggregate", needs=needs)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('"failed"', result.stdout)

    def test_invalid_environment_json_is_a_failed_check(self):
        env = dict(os.environ, P00_CI_NEEDS="{")
        result = subprocess.run([sys.executable, str(GATE), "aggregate"], env=env, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('"failed"', result.stdout)


if __name__ == "__main__": unittest.main()
