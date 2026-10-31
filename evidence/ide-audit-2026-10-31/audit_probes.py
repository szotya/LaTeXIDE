"""Read-only product audit using disposable projects; does not change app source.

Run from the repository root: python3 evidence/ide-audit-2026-10-31/audit_probes.py
The current debug FFI library and local Unix socket permission are required.
"""
from pathlib import Path
import hashlib
import json
import socket
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests"))
from ipc_contract import Runtime, ipc
from compiler_runtime_contract import configure, finished

OUT = Path(__file__).resolve().parent
results = []


def record(identifier, observation, **data):
    result = {"id": identifier, "observation": observation, **data}
    results.append(result)
    print(json.dumps(result, ensure_ascii=False), flush=True)


def replace(runtime, document, revision, before, after):
    assert "\n" not in before
    return runtime.call(op="apply_edits", document_id=document,
                        expected_revision=revision,
                        edits=[{"start": {"line": 0, "character": 0},
                                "end": {"line": 0, "character": len(before)},
                                "text": after}])


def slots():
    with tempfile.TemporaryDirectory(prefix="latexide-audit-slots-", dir="/private/tmp") as temp:
        root = Path(temp)
        for i in range(11):
            (root / f"file{i}.tex").write_text("fixture")
        runtime = Runtime(root)
        try:
            replies = [runtime.call(op="read_document", document_id=f"file{i}.tex") for i in range(11)]
            record("OBS-01", "read_document consumes retained document slots; the eleventh read is rejected",
                   successful_reads=sum(r["ok"] for r in replies), eleventh=replies[-1],
                   open_documents=len(runtime.call(op="workspace_info")["result"]["open_documents"]))
            assert replies[-1]["error"]["code"] == "document_limit"
        finally:
            runtime.close()


def history_and_restart():
    with tempfile.TemporaryDirectory(prefix="latexide-audit-history-", dir="/private/tmp") as temp:
        root = Path(temp)
        (root / "main.tex").write_text("")
        runtime = Runtime(root)
        try:
            assert runtime.call(op="open_document", path="main.tex")["ok"]
            for i in range(300):
                reply = runtime.call(op="apply_edits", document_id="main.tex", expected_revision=i + 1,
                                     edits=[{"start": {"line": 0, "character": i},
                                             "end": {"line": 0, "character": i}, "text": "x"}])
                assert reply["ok"], reply
            undos = 0
            while True:
                reply = runtime.call(op="undo")
                if not reply["ok"]:
                    break
                undos += 1
            text = runtime.call(op="read_document", document_id="main.tex")["result"]["text"]
            record("OBS-02", "300 individual edits retain only 256 undo operations",
                   edits=300, successful_undos=undos, remaining_characters=len(text), terminal_error=reply["error"]["code"])
            assert undos == 256 and len(text) == 44
        finally:
            runtime.close()
        reopened = Runtime(root)
        try:
            text = reopened.call(op="read_document", document_id="main.tex")["result"]["text"]
            record("OBS-03", "A newly created runtime reloads disk; unsaved text/history are not restored",
                   reopened_characters=len(text), can_undo=reopened.call(op="workspace_info")["result"]["can_undo"],
                   scope="runtime close/reopen, not a GUI crash-injection experiment")
            assert text == ""
        finally:
            reopened.close()


def external_change():
    with tempfile.TemporaryDirectory(prefix="latexide-audit-disk-", dir="/private/tmp") as temp:
        root = Path(temp)
        (root / "main.tex").write_text("before")
        runtime = Runtime(root)
        try:
            runtime.call(op="open_document", path="main.tex")
            (root / "main.tex").write_text("external")
            current = runtime.call(op="read_document", document_id="main.tex")["result"]
            save = runtime.call(op="save_document", document_id="main.tex", expected_revision=1)
            record("OBS-04", "An external change leaves the open buffer unchanged; save correctly rejects overwriting it",
                   buffer=current["text"], dirty=current["dirty"], disk=(root / "main.tex").read_text(),
                   save_error=save["error"]["code"])
            assert current["text"] == "before" and save["error"]["code"] == "external_change"
        finally:
            runtime.close()


def invalid_pdf():
    with tempfile.TemporaryDirectory(prefix="latexide-audit-pdf-", dir="/private/tmp") as temp:
        root = Path(temp)
        (root / "main.tex").write_text("fixture")
        program = root / "controlled-compiler"
        program.write_text('#!/bin/sh\nout=""\nfor arg in "$@"; do\n case "$arg" in -outdir=*) out=${arg#-outdir=};; esac\ndone\nprintf abcde > "$out/main.pdf"\n')
        program.chmod(0o755)
        runtime = Runtime(root)
        try:
            runtime.call(op="open_document", path="main.tex")
            assert runtime.call(op="configure_compiler", compiler_path=str(program),
                                cache_directory=str(root / "cache"), output_directory=str(root / "builds"),
                                backend="texlive")["ok"]
            operation = runtime.call(op="request_build", main_file="main.tex")["result"]["id"]
            status = finished(runtime, operation)
            pdf = Path(status["pdf_path"]).read_bytes()
            record("OBS-05", "A zero-exit controlled compiler writing five non-PDF bytes is reported as succeeded",
                   state=status["state"], file_bytes=len(pdf), starts_with_pdf_header=pdf.startswith(b"%PDF-"),
                   scope="controlled compiler output validation; not a TeX Live failure")
            assert status["state"] == "succeeded" and not pdf.startswith(b"%PDF-")
        finally:
            runtime.close()


def dependency_freshness():
    with tempfile.TemporaryDirectory(prefix="latexide-audit-dependencies-", dir="/private/tmp") as temp:
        root = Path(temp)
        source = "\\documentclass{article}\n\\begin{document}\n\\input{chapter}\n\\end{document}\n"
        (root / "main.tex").write_text(source)
        (root / "chapter.tex").write_text("Before.\n")
        runtime = Runtime(root)
        try:
            runtime.call(op="open_document", path="main.tex")
            assert configure(runtime, root)["ok"]
            operation = runtime.call(op="request_build", main_file="main.tex")["result"]["id"]
            status = finished(runtime, operation)
            assert status["state"] == "succeeded", status["log"]
            pdf_hash = hashlib.sha256(Path(status["pdf_path"]).read_bytes()).hexdigest()
            (root / "chapter.tex").write_text("Changed externally after compile.\n")
            updated = runtime.call(op="get_operation", id=operation)["result"]
            record("OBS-06", "A real successful PDF remains stale=false after an unopened input file changes on disk",
                   state=updated["state"], stale=updated["stale"],
                   source_revisions=updated["source_revisions"], pdf_sha256=pdf_hash,
                   scope="actual TeX Live compile, one unopened input dependency")
            assert updated["stale"] is False
        finally:
            runtime.close()


def ipc_stall():
    with tempfile.TemporaryDirectory(prefix="latexide-audit-ipc-", dir="/private/tmp") as temp:
        runtime = Runtime(Path(temp))
        idle = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        second = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            descriptor_path = runtime.call(op="workspace_info")["result"]["session_descriptor"]
            descriptor = json.loads(Path(descriptor_path).read_text())
            idle.connect(descriptor["endpoint"])
            idle.sendall(b'{"version":')
            second.settimeout(0.5)
            second.connect(descriptor["endpoint"])
            envelope = {"version": 1, "token": descriptor["token"], "request": {"op": "workspace_info"}}
            second.sendall(json.dumps(envelope).encode() + b"\n")
            started = time.monotonic()
            try:
                reply = second.recv(1024)
                timed_out = False
            except socket.timeout:
                reply = b""
                timed_out = True
            elapsed = round(time.monotonic() - started, 3)
            idle.close()
            second.settimeout(3)
            if not reply:
                reply = second.recv(8192)
            record("OBS-07", "An incomplete first client frame blocks a second valid client until the first disconnects",
                   valid_client_timed_out=timed_out, wait_seconds=elapsed,
                   recovered_after_first_disconnect=json.loads(reply)["ok"],
                   scope="own local Unix socket, no shutdown-hang or remote exploit claim")
            assert timed_out
        finally:
            idle.close()
            second.close()
            runtime.close()


if __name__ == "__main__":
    for probe in [slots, history_and_restart, external_change, invalid_pdf, dependency_freshness, ipc_stall]:
        probe()
    report = {"scope": "Disposable Mac debug runtime probes; no production source changes", "observations": results}
    (OUT / "audit-probe-results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(f"Recorded {len(results)} observations.", flush=True)
