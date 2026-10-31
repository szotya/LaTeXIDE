"""Exercise the real native library and its local same-session IPC endpoint."""
import ctypes
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = ROOT / "target/debug/liblatexide_ffi.dylib"


class Runtime:
    def __init__(self, root):
        self.lib = ctypes.CDLL(str(LIBRARY))
        self.lib.latexide_runtime_new.argtypes = [ctypes.c_char_p]
        self.lib.latexide_runtime_new.restype = ctypes.c_void_p
        self.lib.latexide_dispatch.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.latexide_dispatch.restype = ctypes.c_void_p
        self.lib.latexide_string_free.argtypes = [ctypes.c_void_p]
        self.lib.latexide_runtime_free.argtypes = [ctypes.c_void_p]
        self.handle = self.lib.latexide_runtime_new(str(root).encode())
        assert self.handle

    def call(self, **request):
        pointer = self.lib.latexide_dispatch(self.handle, json.dumps(request).encode())
        try:
            return json.loads(ctypes.string_at(pointer))
        finally:
            self.lib.latexide_string_free(pointer)

    def close(self):
        self.lib.latexide_runtime_free(self.handle)
        self.handle = None


def ipc(descriptor, request, token=None):
    envelope = {"version": 1, "token": descriptor["token"] if token is None else token, "request": request}
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        stream.settimeout(3)
        stream.connect(descriptor["endpoint"])
        stream.sendall(json.dumps(envelope).encode() + b"\n")
        with stream.makefile("rb") as incoming:
            return json.loads(incoming.readline(1024 * 1024 + 1))


class IpcContract(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "main.tex").write_text("abc")
        self.runtime = Runtime(self.root)
        self.runtime.call(op="open_document", path="main.tex")
        info = self.runtime.call(op="workspace_info")["result"]
        self.descriptor_path = Path(info["session_descriptor"])
        self.descriptor = json.loads(self.descriptor_path.read_text())

    def tearDown(self):
        self.runtime.close()
        self.assertFalse(self.descriptor_path.exists())
        self.temp.cleanup()

    def test_wrong_session_token_is_rejected(self):
        result = ipc(self.descriptor, {"op": "workspace_info"}, "incorrect")
        self.assertFalse(result["ok"])
        self.assertEqual(result["error"]["code"], "unauthorized")

    def test_definition_query_resolves_dirty_exact_target_without_editing_or_opening_buffers(self):
        (self.root / "defs.tex").write_text("\\newcommand{\\mean}{}")
        self.runtime.call(op="apply_edits", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":3},"text":"\\mean"}])
        self.runtime.call(op="open_document", path="defs.tex")
        ipc(self.descriptor, {"op":"apply_edits", "document_id":"defs.tex", "expected_revision":1,
            "edits":[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"😀\n"}]})
        before = self.runtime.call(op="workspace_info")["result"]
        query = {"op":"get_definition", "document_id":"main.tex", "expected_revision":2, "offset_utf16":2}
        result = ipc(self.descriptor, query)
        self.assertTrue(result["ok"], result)
        target = result["result"]["target"]
        self.assertEqual(result["result"]["symbol"], "\\mean")
        self.assertEqual(target, {"document_id":"defs.tex", "revision":2, "start_utf16":15, "length_utf16":5, "line":2})
        self.assertEqual(self.runtime.call(op="workspace_info")["result"], before)
        stale = ipc(self.descriptor, {**query, "expected_revision":1})
        self.assertEqual(stale["error"]["code"], "revision_conflict")
        self.assertTrue(self.runtime.call(op="undo")["ok"])
        restored = ipc(self.descriptor, query)["result"]["target"]
        self.assertEqual((restored["start_utf16"], restored["revision"]), (12, 3))
        self.assertEqual((self.root / "defs.tex").read_text(), "\\newcommand{\\mean}{}")

    def test_syntax_window_uses_dirty_text_without_changing_history_or_revision(self):
        source = "\\section{Árvíz 😀}\n% comment\n"
        edited = self.runtime.call(op="apply_edits", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":3},"text":source}])
        self.assertTrue(edited["ok"])
        syntax = self.runtime.call(op="get_syntax", document_id="main.tex", expected_revision=2,
            offset_utf16=0, length_utf16=200)
        self.assertTrue(syntax["ok"], syntax)
        self.assertIn([0, 8, 1], syntax["result"]["spans"])
        current = self.runtime.call(op="read_document", document_id="main.tex")["result"]
        self.assertEqual(current["text"], source)
        self.assertEqual(current["revision"], 2)
        self.assertEqual(current["line_ending"], "lf")
        stale = self.runtime.call(op="get_syntax", document_id="main.tex", expected_revision=1, offset_utf16=0, length_utf16=20)
        self.assertEqual(stale["error"]["code"], "revision_conflict")
        self.assertTrue(self.runtime.call(op="undo")["ok"])
        self.assertEqual(self.runtime.call(op="read_document", document_id="main.tex")["result"]["text"], "abc")

    def test_completion_uses_authoritative_dirty_source_and_rejects_stale_revision(self):
        edit=self.runtime.call(op="apply_edits", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":3},"text":"😀\\sec"}])
        self.assertTrue(edit["ok"],edit)
        reply=ipc(self.descriptor,{"op":"get_completions","document_id":"main.tex","expected_revision":2,"caret_utf16":6})
        self.assertTrue(reply["ok"],reply)
        completion=reply["result"]
        self.assertEqual((completion["replace_start_utf16"],completion["replace_length_utf16"]),(2,4))
        self.assertEqual(completion["items"][0]["insert_text"],"\\section{}")
        self.assertEqual(completion["revision"],2)
        current=self.runtime.call(op="read_document",document_id="main.tex")["result"]
        self.assertEqual(current["text"],"😀\\sec");self.assertEqual(current["revision"],2)
        stale=self.runtime.call(op="get_completions",document_id="main.tex",expected_revision=1,caret_utf16=6)
        self.assertEqual(stale["error"]["code"],"revision_conflict")
        self.assertTrue(self.runtime.call(op="undo")["ok"])
        self.assertEqual(self.runtime.call(op="read_document",document_id="main.tex")["result"]["text"],"abc")

    def test_project_completion_tracks_another_dirty_agent_document_without_mutating_the_query(self):
        (self.root / "labels.tex").write_text("\\label{sec:old}")
        self.runtime.call(op="open_document", path="labels.tex")
        self.runtime.call(op="apply_edits", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":3},"text":"\\ref{sec:}"}])
        ipc(self.descriptor,{"op":"apply_edits","document_id":"labels.tex","expected_revision":1,
            "edits":[{"start":{"line":0,"character":11},"end":{"line":0,"character":14},"text":"new"}]})
        query={"op":"get_completions","document_id":"main.tex","expected_revision":2,"caret_utf16":9}
        reply=ipc(self.descriptor,query)
        self.assertTrue(reply["ok"],reply)
        self.assertEqual([item["label"] for item in reply["result"]["items"]],["sec:new"])
        self.assertEqual(reply["result"]["items"][0]["kind"],"reference")
        self.assertEqual(self.runtime.call(op="read_document",document_id="main.tex")["result"]["revision"],2)
        self.runtime.call(op="undo")
        self.assertEqual([item["label"] for item in ipc(self.descriptor,query)["result"]["items"]],["sec:old"])
        self.assertEqual((self.root / "labels.tex").read_text(),"\\label{sec:old}")

    def test_forced_project_refresh_does_not_trust_unchanged_disk_metadata(self):
        labels=self.root / "labels.tex"
        labels.write_text("\\label{sec:old}")
        self.runtime.call(op="apply_edits",document_id="main.tex",expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":3},"text":"\\ref{sec:}"}])
        query={"op":"get_completions","document_id":"main.tex","expected_revision":2,"caret_utf16":9}
        self.assertEqual(ipc(self.descriptor,query)["result"]["items"][0]["label"],"sec:old")
        metadata=labels.stat();labels.write_text("\\label{sec:new}")
        os.utime(labels,ns=(metadata.st_atime_ns,metadata.st_mtime_ns))
        result=ipc(self.descriptor,{**query,"refresh_project":True})
        self.assertTrue(result["ok"],result)
        self.assertEqual([item["label"] for item in result["result"]["items"]],["sec:new"])

    def test_composition_blocks_remote_edits_then_commits_once(self):
        started = self.runtime.call(op="begin_composition", document_id="main.tex", expected_revision=1)
        self.assertTrue(started["ok"], started)
        change = {"op":"apply_edits", "document_id":"main.tex", "expected_revision":1,
            "edits":[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"agent"}]}
        denied = ipc(self.descriptor, change)
        self.assertEqual(denied["error"]["code"], "composition_active")
        self.assertEqual(self.runtime.call(op="request_build", main_file="main.tex")["error"]["code"], "composition_active")
        committed = self.runtime.call(op="end_composition", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"日本語"}])
        self.assertTrue(committed["ok"], committed)
        self.assertEqual(committed["result"]["revision"], 2)
        self.assertTrue(self.runtime.call(op="undo")["ok"])
        self.assertEqual(self.runtime.call(op="read_document", document_id="main.tex")["result"]["text"], "abc")

    def test_rejected_composition_releases_the_workspace_for_further_edits(self):
        initial = self.runtime.call(op="apply_edits", document_id="main.tex", expected_revision=1,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"x" * (1024 * 1024 - 1000)}])
        self.assertTrue(initial["ok"], initial)
        self.assertTrue(self.runtime.call(op="begin_composition", document_id="main.tex", expected_revision=2)["ok"])
        rejected = self.runtime.call(op="end_composition", document_id="main.tex", expected_revision=2,
            edits=[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"x" * 2000}])
        self.assertFalse(rejected["ok"])
        changed = ipc(self.descriptor, {"op":"apply_edits", "document_id":"main.tex", "expected_revision":2,
            "edits":[{"start":{"line":0,"character":0},"end":{"line":0,"character":0},"text":"next "}]})
        self.assertTrue(changed["ok"], changed)

    def test_agent_edit_is_visible_to_native_runtime_and_undoable_after_save(self):
        result = ipc(self.descriptor, {"op": "apply_edits", "document_id": "main.tex", "expected_revision": 1,
            "edits": [{"start": {"line": 0, "character": 1}, "end": {"line": 0, "character": 1}, "text": "😀"}]})
        self.assertTrue(result["ok"], result)
        self.assertEqual(self.runtime.call(op="read_document", document_id="main.tex")["result"]["text"], "a😀bc")
        self.assertTrue(ipc(self.descriptor, {"op": "save_document", "document_id": "main.tex", "expected_revision": 2})["ok"])
        self.assertEqual((self.root / "main.tex").read_text(), "a😀bc")
        self.assertTrue(self.runtime.call(op="undo")["ok"])
        self.assertEqual(self.runtime.call(op="read_document", document_id="main.tex")["result"]["text"], "abc")

    def test_parent_traversal_is_rejected(self):
        result = ipc(self.descriptor, {"op": "open_document", "path": "../private.tex"})
        self.assertFalse(result["ok"])
        self.assertEqual(result["error"]["code"], "path_escape")


if __name__ == "__main__":
    unittest.main()
