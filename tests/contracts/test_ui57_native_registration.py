"""Native-run registration only; mocked command output is not UI acceptance."""
from pathlib import Path
import importlib.util
import json
import os
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("ui57_registration_probes", ROOT / "integration/registry/probes.py")
probes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probes)

SUMMARY = "Test Suite 'All tests' passed at 2026-11-16.\n    Executed 1 test, with 0 failures (0 unexpected) in 0.01 seconds\n"

class UI57NativeRegistrationTests(unittest.TestCase):
    def test_render_output_is_owned_and_inherited_optins_are_cleared(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            result = root / "native-controls.json"
            foreign = root / "foreign-render-output"
            def native_command(argv, env):
                owned = root / "ui57-titlebar-renders"
                self.assertEqual(Path(env["UI57_RENDER_DIR"]), owned)
                self.assertTrue(owned.is_dir())
                if os.name == "posix":
                    self.assertEqual(owned.stat().st_mode & 0o777, 0o700)
                self.assertNotIn("LATEXIDE_RENDER_PREVIEWS", env)
                self.assertNotIn("LATEXIDE_CODEX_HANDSHAKE", env)
                self.assertNotIn("UI57_CASE", env)
                self.assertIn("--skip-build", argv)
                self.assertFalse(foreign.exists())
                return SimpleNamespace(stdout=SUMMARY, returncode=0)
            with mock.patch.object(probes, "mac_required"), mock.patch.object(probes, "command_result", side_effect=native_command), mock.patch.object(sys, "argv", ["probes.py", "native-controls", "--result", str(result)]), mock.patch.dict(os.environ, {"UI57_RENDER_DIR": str(foreign), "LATEXIDE_RENDER_PREVIEWS": "1", "LATEXIDE_CODEX_HANDSHAKE": "1", "UI57_CASE": "../../foreign-case"}):
                self.assertEqual(probes.main(), 0)
            self.assertEqual(json.loads(result.read_text())["passed"], 1)
            self.assertFalse(foreign.exists())

    def test_existing_render_path_is_refused_before_native_command(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); owned = root / "ui57-titlebar-renders"
            owned.mkdir(mode=0o700); sentinel = owned / "retained.txt"
            sentinel.write_bytes(b"prior output must remain unchanged")
            with mock.patch.object(probes, "mac_required"), mock.patch.object(probes, "command_result", return_value=SimpleNamespace(stdout=SUMMARY, returncode=0)) as native, mock.patch.object(sys, "argv", ["probes.py", "native-controls", "--result", str(root / "native-controls.json")]):
                self.assertEqual(probes.main(), 1)
                native.assert_not_called()
            self.assertEqual(sentinel.read_bytes(), b"prior output must remain unchanged")
            self.assertFalse((root / "native-controls.json").exists())

if __name__ == "__main__":
    unittest.main()
