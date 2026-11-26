import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]


def load(relative, name):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SourceCopyGitContextTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.verify = load("integration/registry/verify.py", "copy_context_verify")
        cls.checker = load("scripts/check_repository_content.py", "copy_context_checker")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="git-context-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def git_output(self, top, names=b"file.txt\0", content=b"plain source\n"):
        def output(argv, **kwargs):
            self.assertEqual(Path(kwargs["cwd"]).resolve(), self.root)
            if argv[1:] == ["rev-parse", "--show-toplevel"]:
                return str(top) + "\n"
            if argv[1:] == ["rev-parse", "HEAD"]:
                return "b" * 40 + "\n"
            if argv[1:3] == ["status", "--porcelain"]:
                return b" M file.txt\0"
            if argv[1:] in (["ls-files", "-z"], ["ls-tree", "-r", "--name-only", "-z", "HEAD"]):
                return names
            if argv[1:] == ["show", "HEAD:file.txt"]:
                return content
            raise AssertionError("Unexpected read-only Git command: " + repr(argv))
        return output

    def check(self, output, mode="--tracked"):
        stream = io.StringIO()
        with mock.patch.object(self.checker, "ROOT", self.root), \
             mock.patch.object(self.checker.subprocess, "check_output", side_effect=output), \
             mock.patch("sys.argv", ["content-check", mode]), \
             contextlib.redirect_stdout(stream):
            code = self.checker.main()
        return code, json.loads(stream.getvalue())

    def test_parent_repository_is_not_source_copy_identity(self):
        with mock.patch.object(self.verify.subprocess, "check_output",
                               side_effect=self.git_output(self.root.parent)):
            self.assertEqual(self.verify.repository_identity(self.root),
                             {"head": None, "dirty": None})

    def test_own_repository_identity_is_retained(self):
        with mock.patch.object(self.verify.subprocess, "check_output",
                               side_effect=self.git_output(self.root)):
            self.assertEqual(self.verify.repository_identity(self.root),
                             {"head": "b" * 40, "dirty": True})

    def test_parent_repository_cannot_supply_empty_tracked_pass(self):
        code, data = self.check(self.git_output(self.root.parent, names=b""))
        self.assertNotEqual(code, 0)
        self.assertEqual(data["result"], "FAIL")

    def test_empty_owned_candidate_is_not_pass(self):
        code, data = self.check(self.git_output(self.root, names=b""))
        self.assertNotEqual(code, 0)
        self.assertEqual(data["result"], "FAIL")

    def test_staged_deletions_have_no_remaining_content_to_scan(self):
        def output(argv, **kwargs):
            self.assertEqual(Path(kwargs["cwd"]).resolve(), self.root)
            if argv[1:] == ["rev-parse", "--show-toplevel"]:
                return str(self.root) + "\n"
            if argv[1:] == ["diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z"]:
                return b""  # A deletion-only index has no surviving ACMR blobs.
            raise AssertionError("Unexpected read-only Git command: " + repr(argv))
        code, data = self.check(output, "--staged")
        self.assertEqual(code, 0)
        self.assertEqual(data["result"], "PASS")
        self.assertEqual((data["files"], data["bytes"], data["findings"]), (0, 0, []))

    def test_nonempty_owned_blob_keeps_content_guards(self):
        for content, expected in [(b"plain source\n", "PASS"),
                                  (b"MZown-inert-pattern-control", "FAIL")]:
            with self.subTest(expected=expected):
                code, data = self.check(self.git_output(self.root, content=content))
                self.assertEqual(data["result"], expected)
                self.assertEqual(code == 0, expected == "PASS")
                self.assertEqual(data["files"], 1)


    def test_tracked_HEAD_inventory_is_not_dirty_index_inventory(self):
        def output(argv, **kwargs):
            self.assertEqual(Path(kwargs["cwd"]).resolve(), self.root)
            if argv[1:] == ["rev-parse", "--show-toplevel"]:
                return str(self.root) + "\n"
            if argv[1:] == ["ls-files", "-z"]:
                return b"safe.txt\0"
            if argv[1:] == ["ls-tree", "-r", "--name-only", "-z", "HEAD"]:
                return b"safe.txt\0payload.bin\0"
            if argv[1:] == ["show", "HEAD:safe.txt"]:
                return b"plain source\n"
            if argv[1:] == ["show", "HEAD:payload.bin"]:
                return b"MZvirtual-HEAD-forbidden-object"
            raise AssertionError("Unexpected read-only Git command: " + repr(argv))
        code, data = self.check(output)
        self.assertNotEqual(code, 0)
        self.assertEqual(data["result"], "FAIL")
        self.assertEqual(data["files"], 2)
        self.assertIn({"path": "payload.bin", "reason": "compiled_executable"}, data["findings"])


if __name__ == "__main__":
    unittest.main()
