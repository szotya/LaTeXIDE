"""Private P19 test producer. No real P01/P03 implementation is invoked.

The checkpoint exit is an ordinary self-exit (86), not kernel power loss.
All paths are beneath the private directory supplied by the owning harness.
No subprocess, IPC listener, native app, compiler or external project is used.
"""
import argparse
import json
import os
from pathlib import Path
import struct
import sys
import time


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def frame(value):
    raw = encoded(value)
    return struct.pack(">I", len(raw)) + raw


def synced(path, raw):
    with path.open("wb") as stream:
        stream.write(raw); stream.flush(); os.fsync(stream.fileno())


def recover_journal(raw):
    """Own synthetic framing model: incomplete tail cannot become a record."""
    offset = 0; last = None; truncated = False
    while offset < len(raw):
        if len(raw) - offset < 4:
            truncated = True; break
        length = struct.unpack(">I", raw[offset:offset + 4])[0]
        if not 0 < length <= 1048576:
            raise ValueError("journal_quota")
        if offset + 4 + length > len(raw):
            truncated = True; break
        record = json.loads(raw[offset + 4:offset + 4 + length])
        if set(record) != {"schema", "revision", "files"} or type(record["schema"]) is not int or record["schema"] != 1:
            raise ValueError("journal_schema")
        if type(record["revision"]) is not int or not 0 <= record["revision"] < 2**64 or (last and record["revision"] <= last["revision"]):
            raise ValueError("journal_revision")
        if not isinstance(record["files"], dict) or not record["files"]:
            raise ValueError("journal_files")
        for name, values in record["files"].items():
            if name.startswith("/") or any(p in ("", ".", "..") for p in name.split("/")):
                raise ValueError("journal_path")
            if not isinstance(values, list) or any(type(v) is not int or not 0 <= v <= 255 for v in values):
                raise ValueError("journal_bytes")
        last = record; offset += 4 + length
    if last is None:
        raise ValueError("no_complete_journal_record")
    return last, truncated


def emit(request, phase, recovered_revision=None, truncated=False):
    event = {**request["binding"], "schema": 1, "phase": phase,
             "seq": 1 if phase == "pre_fault" else 2, "case_count": 1,
             "partial_edit_allowed": False, "recovered_revision": recovered_revision,
             "truncated_tail_rejected": truncated}
    if phase == "pre_fault":
        if request["case"]["mode"] == "wrong_nonce": event["nonce"] = "0" * 32
        if request["case"]["mode"] == "bad_schema": event["schema"] = 2
        if request["case"]["mode"] == "wrong_seq": event["seq"] = 3
        if request["case"]["mode"] == "zero_claim": event["case_count"] = 0
        if request["case"]["mode"] == "truncated_log":
            sys.stdout.buffer.write(b'{"schema":1'); sys.stdout.buffer.flush(); return
    elif request["case"]["mode"] == "bool_revision":
        event["recovered_revision"] = True
    sys.stdout.buffer.write(encoded(event) + b"\n"); sys.stdout.buffer.flush()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--recover", action="store_true")
    args = parser.parse_args()
    request = json.loads(args.request.read_bytes())
    directory = args.request.resolve().parent
    if Path.cwd().resolve() != directory:
        raise ValueError("private_directory_mismatch")
    case = request["case"]; mode = case["mode"]; ack = case["acknowledged_revision"]
    journal = directory / "journal.bin"
    if args.recover:
        recovered, truncated = recover_journal(journal.read_bytes())
        if mode == "wrong_revision": recovered["revision"] = ack - 1
        if mode == "partial_recovered":
            name = next(iter(recovered["files"]))
            recovered["files"][name] = request["before"][name]
        (directory / "recovered.json").write_bytes(encoded(recovered))
        if mode != "missing_post": emit(request, "post_recovery", recovered["revision"], truncated)
        return 0
    if mode == "timeout": time.sleep(5)
    if mode == "zero_case": return 0
    record = {"schema": 1, "revision": ack, "files": request["after"]}
    if mode == "bad_journal_schema": record["schema"] = True
    if mode == "bad_journal_revision": record["revision"] = True
    raw = frame(record)
    # The named BeforeJournalSync point concerns an unacknowledged next
    # record. ack already belongs to the last durable record in this model.
    if case["point"] == "before_journal_sync":
        (directory / "unacknowledged.tmp").write_bytes(frame({**record, "revision": ack + 1})[:9])
    if mode == "journal_tail": raw += struct.pack(">I", 999) + b'{"schema":1'
    if mode == "truncated_journal": raw = raw[:9]
    synced(journal, raw)
    if case["point"] == "before_rename": (directory / "save.tmp").write_bytes(encoded(request["after"]))
    if case["point"] == "mid_frame": (directory / "ipc.frame").write_bytes(struct.pack(">I", 100) + b'{')
    if case["point"] == "after_rename":
        for name, values in request["after"].items():
            if mode == "partial_edit" and not name.endswith("main.tex"): continue
            synced(directory / "project" / name, bytes(values))
    emit(request, "pre_fault")
    if mode == "skip": return 77
    if mode == "unexpected_exit": return 0
    os._exit(86)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(type(error).__name__ + ": " + str(error), file=sys.stderr)
        sys.exit(2)
