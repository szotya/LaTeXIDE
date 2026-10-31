import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from ipc_contract import Runtime, ipc

ROOT = Path(__file__).resolve().parents[1]


def configure(runtime, root, compiler=None):
    latexmk = ROOT / ".local/texlive-macos/bin/universal-darwin/latexmk"
    backend = "texlive" if compiler is None and latexmk.exists() else "tectonic"
    selected = compiler or (latexmk if backend == "texlive" else ROOT / ".local/compilers/macos-arm64/tectonic")
    cache = ROOT / (".local/texlive-user-cache" if backend == "texlive" else ".local/tectonic-cache")
    return runtime.call(op="configure_compiler", compiler_path=str(selected),
        cache_directory=str(cache), output_directory=str(root / "builds"), backend=backend)


def finished(runtime, operation):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        reply = runtime.call(op="get_operation", id=operation)
        if not reply["ok"]:
            raise AssertionError(reply)
        if reply["result"]["state"] not in ("queued", "running"):
            return reply["result"]
        time.sleep(0.03)
    raise AssertionError("Compiler did not finish within 15 seconds")


class CompilerRuntimeContract(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="latexide-compiler-", dir="/private/tmp")
        self.root = Path(self.temp.name)
        shutil.copy(ROOT / "tests/fixtures/basic/main.tex", self.root / "main.tex")
        self.runtime = Runtime(self.root)
        self.runtime.call(op="open_document", path="main.tex")

    def tearDown(self):
        self.runtime.close()
        self.temp.cleanup()

    def test_actual_compiler_publishes_a_pdf_and_tracks_source_revision(self):
        compiler = ROOT / ".local/texlive-macos/bin/universal-darwin/latexmk"
        self.assertTrue(compiler.exists(), "This acceptance run requires the verified TeX Live installation")
        print("Compiler acceptance: TeX Live 2026 / latexmk4.88 / XeLaTeX; bin alias=" + str(compiler), flush=True)
        self.assertTrue(configure(self.runtime, self.root)["ok"])
        reply = self.runtime.call(op="request_build", main_file="main.tex")
        self.assertTrue(reply["ok"], reply)
        status = finished(self.runtime, reply["result"]["id"])
        self.assertEqual(status["state"], "succeeded", status["log"])
        self.assertTrue(Path(status["pdf_path"]).read_bytes().startswith(b"%PDF-"))
        self.assertFalse(status["stale"])
        self.runtime.call(op="apply_edits", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"% edit\n"}])
        status = self.runtime.call(op="get_operation", id=reply["result"]["id"])["result"]
        self.assertTrue(status["stale"])
        denied = self.runtime.call(op="request_build", main_file="main.tex")
        self.assertEqual(denied["error"]["code"], "needs_save")

    def test_agent_cannot_replace_the_compiler_with_an_arbitrary_program(self):
        info = self.runtime.call(op="workspace_info")["result"]
        descriptor = json.loads(Path(info["session_descriptor"]).read_text())
        reply = ipc(descriptor, {"op":"configure_compiler", "compiler_path":"/usr/bin/true", "cache_directory":str(self.root),
            "output_directory":str(self.root), "backend":"tectonic"})
        self.assertFalse(reply["ok"])
        self.assertEqual(reply["error"]["code"], "permission_denied")

    def test_compiler_does_not_inherit_agent_credentials(self):
        environment = dict(os.environ, LATEXIDE_FAKE_TOKEN="nonsecret-test-sentinel")
        result = subprocess.run([sys.executable, __file__, "--environment-probe"], env=environment,
            text=True, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


def environment_probe():
    with tempfile.TemporaryDirectory(prefix="latexide-env-", dir="/private/tmp") as temp:
        root = Path(temp)
        (root / "main.tex").write_text("fixture")
        program = root / "compiler"
        program.write_text('#!/bin/sh\nif [ -n "$LATEXIDE_FAKE_TOKEN" ]; then echo unsafe_env=present; else echo unsafe_env=absent; fi\nexit 1\n')
        program.chmod(0o755)
        runtime = Runtime(root)
        try:
            assert configure(runtime, root, program)["ok"]
            result = runtime.call(op="request_build", main_file="main.tex")
            status = finished(runtime, result["result"]["id"])
            assert "unsafe_env=absent" in status["log"], "Compiler inherited a credential-like environment variable"
        finally:
            runtime.close()


if __name__ == "__main__":
    if "--environment-probe" in sys.argv:
        environment_probe()
    else:
        unittest.main()
