"""Real stdio MCP client -> bridge -> the native process's existing runtime."""
import json
from pathlib import Path
import selectors
import subprocess
import tempfile
import unittest
from ipc_contract import Runtime

ROOT = Path(__file__).resolve().parents[1]


class MCPContract(unittest.TestCase):
    def test_dirty_read_direct_edit_save_and_shared_undo(self):
        with tempfile.TemporaryDirectory(prefix="latexide-mcp-", dir="/private/tmp") as folder:
            root = Path(folder)
            (root / "main.tex").write_text("Árvíz 😀\n")
            runtime = Runtime(root)
            process = None
            try:
                runtime.call(op="open_document", path="main.tex")
                native_edit = dict(op="apply_edits", document_id="main.tex", expected_revision=1,
                    edits=[dict(start=dict(line=0, character=0), end=dict(line=0, character=0), text="user ")])
                self.assertTrue(runtime.call(**native_edit)["ok"])
                descriptor = runtime.call(op="workspace_info")["result"]["session_descriptor"]
                process = subprocess.Popen([str(ROOT / "target/debug/latexide-mcp"), "--session", descriptor],
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
                sequence = 0

                def rpc(method, params):
                    nonlocal sequence
                    sequence += 1
                    packet = dict(jsonrpc="2.0", id=sequence, method=method, params=params)
                    process.stdin.write(json.dumps(packet).encode() + b"\n")
                    with selectors.DefaultSelector() as selector:
                        selector.register(process.stdout, selectors.EVENT_READ)
                        data = bytearray()
                        while b"\n" not in data:
                            self.assertTrue(selector.select(8), "MCP reply timed out")
                            chunk = process.stdout.read(1)
                            self.assertTrue(chunk, "MCP exited before a reply")
                            data.extend(chunk)
                    reply = json.loads(data)
                    self.assertEqual(reply["id"], sequence)
                    self.assertNotIn("error", reply, reply)
                    return reply["result"]

                initialized = rpc("initialize", dict(protocolVersion="2025-11-25",
                    capabilities={}, clientInfo=dict(name="latexide-contract", version="1")))
                self.assertIn("tools", initialized["capabilities"])
                process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
                listed = rpc("tools/list", {})
                names = {tool["name"] for tool in listed["tools"]}
                self.assertTrue({"read_document", "apply_edits", "save_document", "request_build"} <= names)
                self.assertNotIn("configure_compiler", names)

                def tool(name, arguments):
                    result = rpc("tools/call", dict(name=name, arguments=arguments))
                    return json.loads(result["content"][0]["text"])

                text = tool("read_document", dict(document_id="main.tex"))
                self.assertEqual(text["result"]["text"], "user Árvíz 😀\n")
                edited = tool("apply_edits", dict(document_id="main.tex", expected_revision=2,
                    edits=[dict(start=dict(line=0, character=5), end=dict(line=0, character=10), text="agent")]))
                self.assertTrue(edited["ok"], edited)
                self.assertEqual(runtime.call(op="read_document", document_id="main.tex")["result"]["text"], "user agent 😀\n")
                saved = tool("save_document", dict(document_id="main.tex", expected_revision=3))
                self.assertTrue(saved["ok"])
                self.assertEqual((root / "main.tex").read_text(), "user agent 😀\n")
                self.assertTrue(runtime.call(op="undo")["ok"])
                restored = tool("read_document", dict(document_id="main.tex"))["result"]
                self.assertEqual(restored["text"], "user Árvíz 😀\n")
                self.assertTrue(restored["dirty"])
                conflict = tool("apply_edits", dict(document_id="main.tex", expected_revision=3, edits=[]))
                self.assertEqual(conflict["error"]["code"], "revision_conflict")
            finally:
                if process is not None:
                    process.stdin.close()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                    process.stdout.close()
                    process.stderr.close()
                runtime.close()


if __name__ == "__main__":
    unittest.main()
