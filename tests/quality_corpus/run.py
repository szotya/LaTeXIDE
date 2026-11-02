"""P19 QualityCorpusPort adapter; no recovery/runtime/compiler provider."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MAX_BYTES = 1024 * 1024
MAX_CASES = 16
MAX_BUDGET_MS = 3000
POINTS = ("before_journal_sync", "after_journal_sync", "before_rename",
          "after_rename", "mid_frame")
MODES = ("normal", "timeout", "zero_case", "skip", "wrong_nonce", "bad_schema",
         "missing_post", "wrong_revision", "truncated_journal", "unexpected_exit",
         "partial_edit", "journal_tail")
MODES += ("bool_revision", "partial_recovered", "wrong_seq", "truncated_log", "zero_claim",
          "bad_journal_schema", "bad_journal_revision")


def _bytes(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _uint(value, maximum=2**64 - 1):
    if type(value) is not int or not 0 <= value <= maximum:
        raise ValueError("invalid_integer")
    return value


def _shape(value, keys):
    if type(value) is not dict or set(value) != set(keys):
        raise ValueError("invalid_schema")


def _json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate_key")
            result[key] = value
        return result

    def constant(_):
        raise ValueError("nonfinite_number")

    try:
        return json.loads(raw, object_pairs_hook=pairs, parse_constant=constant)
    except (UnicodeError, RecursionError) as error:
        raise ValueError("invalid_json") from error


def _relative(path):
    if not isinstance(path, str) or not path or any(c in path for c in "\\:*?<>|\"\0"):
        raise ValueError("invalid_path")
    if any(part in ("", ".", "..") for part in path.split("/")):
        raise ValueError("invalid_path")
    return path


def expected_fault_outcome(point: str, acknowledged_revision: int) -> dict:
    """Acknowledge only durable edits; a save may publish at AfterRename.

    These are P19 oracles, not the DTO's example response or product results.
    The pre-sync point refers to an unacknowledged *next* edit; an existing
    acknowledged revision must still survive. Recovery never grants save.
    """
    if point not in POINTS:
        raise ValueError("invalid_fault_point")
    if type(acknowledged_revision) is not int or not 0 <= acknowledged_revision < 2**64:
        raise ValueError("invalid_revision")
    return {"minimum_recovered_revision": acknowledged_revision,
            "disk_may_change": point == "after_rename",
            "partial_edit_allowed": False}


def decode_contract_request(raw: bytes) -> dict:
    """Strict, lossless Python boundary for the frozen P19 request DTO only."""
    if not isinstance(raw, bytes) or len(raw) > MAX_BYTES:
        raise ValueError("payload_too_large")
    request = _json(raw)
    _shape(request, ("version", "package_id", "request_id", "snapshot_id", "payload"))
    version = request["version"]
    _shape(version, ("major", "minor"))
    if type(version["major"]) is not int or type(version["minor"]) is not int or version != {"major": 1, "minor": 0}:
        raise ValueError("incompatible_version")
    if request["package_id"] != "P19" or not isinstance(request["request_id"], str) or not request["request_id"]:
        raise ValueError("invalid_envelope")
    if request["snapshot_id"] is not None and not isinstance(request["snapshot_id"], str):
        raise ValueError("invalid_envelope")
    _shape(request["payload"], ("point", "acknowledged_revision"))
    return expected_fault_outcome(**request["payload"])


def decode_frame(raw: bytes):
    """Own length-framed JSON control; this is not the production IPC parser."""
    if len(raw) < 4:
        raise ValueError("truncated_frame")
    length = struct.unpack(">I", raw[:4])[0]
    if length > MAX_BYTES:
        raise ValueError("frame_quota")
    if length == 0 or len(raw) != 4 + length:
        raise ValueError("truncated_frame")
    return _json(raw[4:])


def _decode_text(raw):
    bom = raw.startswith(b"\xef\xbb\xbf")
    text = raw[3:].decode("utf-8") if bom else raw.decode("utf-8")
    ending = "crlf" if "\r\n" in text else "lf"
    if "\r" in text.replace("\r\n", "") or (ending == "crlf" and "\n" in text.replace("\r\n", "")):
        raise ValueError("mixed_line_endings")
    return text.replace("\r\n", "\n"), bom, ending


def load_profiles(manifest_path=None):
    """Validate immutable raw/logical identities; never regenerate baselines."""
    path = Path(manifest_path) if manifest_path else HERE / "corpus/manifest.json"
    manifest = _json(path.read_bytes())
    _shape(manifest, ("schema", "seed", "max_cases", "time_budget_ms", "profiles"))
    if manifest["schema"] != 1 or manifest["seed"] != 1901 or manifest["max_cases"] != MAX_CASES or manifest["time_budget_ms"] != MAX_BUDGET_MS:
        raise ValueError("corpus_budget")
    profiles = manifest["profiles"]
    if [p["id"] for p in profiles] != ["singlefile", "multifile-hu"]:
        raise ValueError("corpus_profiles")
    for profile in profiles:
        _relative(profile["id"])
        declared = set()
        for item in profile["files"]:
            name = _relative(item["path"])
            file = path.parent / profile["id"] / name
            if name in declared or file.is_symlink() or not file.resolve().is_relative_to(path.parent.resolve()):
                raise ValueError("fixture_identity")
            declared.add(name)
            raw = file.read_bytes()
            if len(raw) != item["size"] or _sha(raw) != item["sha256"]:
                raise ValueError("fixture_identity")
            if "raw_bytes" in item:
                values = item["raw_bytes"]
                if not isinstance(values, list) or any(type(v) is not int or not 0 <= v <= 255 for v in values):
                    raise ValueError("fixture_identity")
                materialized = bytes(values)
                if len(materialized) != item["raw_size"] or _sha(materialized) != item["raw_sha256"] or _decode_text(materialized)[0] != _decode_text(raw)[0]:
                    raise ValueError("fixture_identity")
                raw = materialized
            if item["role"] in ("tex", "bib"):
                text, bom, ending = _decode_text(raw)
                if (_sha(text.encode()), bom, ending) != (item["logical_sha256"], item["physical_bom"], item["line_ending"]):
                    raise ValueError("fixture_identity")
        actual = {p.relative_to(path.parent / profile["id"]).as_posix() for p in (path.parent / profile["id"]).rglob("*") if p.is_file()}
        if actual != declared or not declared or not profile["missing_data"]:
            raise ValueError("fixture_identity")
        if profile["expected"]["compile_state"] != "unrun" or set(profile["platforms"].values()) != {"unrun"}:
            raise ValueError("unqualified_corpus_claim")
    return profiles


def _utf16_slice(text, start, length):
    _uint(start, 2**32 - 1); _uint(length, 2**32 - 1)
    if start + length > 2**32 - 1:
        raise ValueError("invalid_utf16")
    offsets = {0: 0}; units = 0
    for index, scalar in enumerate(text):
        units += 2 if ord(scalar) > 0xFFFF else 1
        if units in (start, start + length):
            offsets[units] = index + 1
    if start not in offsets or start + length not in offsets:
        raise ValueError("invalid_utf16")
    return offsets[start], offsets[start + length]


def run_model_session(raw: bytes, operations: list, max_steps=16, time_budget_ms=1000):
    """Bounded test-only edit/save/undo model, never a runtime replacement."""
    if not 1 <= max_steps <= MAX_CASES or len(operations) > max_steps or not 1 <= time_budget_ms <= MAX_BUDGET_MS:
        raise ValueError("model_budget")
    text, bom, ending = _decode_text(raw)
    if len(raw) > MAX_BYTES:
        raise ValueError("quota")
    disk = raw; revision = 1; history = []; trace = []
    deadline = time.monotonic() + time_budget_ms / 1000

    def encoded(value):
        value = value.replace("\n", "\r\n") if ending == "crlf" else value
        return (b"\xef\xbb\xbf" if bom else b"") + value.encode("utf-8")

    for seq, operation in enumerate(operations, 1):
        before = (_sha(text.encode()), _sha(disk), revision)
        status = "ok"
        try:
            if time.monotonic() >= deadline:
                raise ValueError("timeout")
            op = operation.get("op") if isinstance(operation, dict) else None
            keys = ("op", "expected_revision", "start_utf16", "length_utf16", "replacement") if op == "agent_edit" else ("op", "expected_revision")
            _shape(operation, keys)
            expected = _uint(operation["expected_revision"])
            if expected != revision:
                raise ValueError("revision_conflict")
            if op == "agent_edit":
                if not isinstance(operation["replacement"], str):
                    raise ValueError("invalid_schema")
                a, b = _utf16_slice(text, operation["start_utf16"], operation["length_utf16"])
                next_text = text[:a] + operation["replacement"] + text[b:]
                if len(next_text.encode()) > MAX_BYTES:
                    raise ValueError("quota")
                _uint(revision + 1)
                history.append(text); text = next_text; revision += 1
            elif op == "explicit_save":
                disk = encoded(text)
            elif op == "undo":
                if not history:
                    raise ValueError("no_undo")
                _uint(revision + 1)
                text = history.pop(); revision += 1
            else:
                raise ValueError("invalid_schema")
        except (ValueError, UnicodeError, TypeError) as error:
            status = str(error) if isinstance(error, ValueError) and not isinstance(error, UnicodeError) else "invalid_schema"
        trace.append({"seq": seq, "op": operation.get("op") if isinstance(operation, dict) else None,
                      "status": status, "revision": revision, "before": before,
                      "memory_sha256": _sha(text.encode()), "disk_sha256": _sha(disk)})
    return {"scope": "synthetic-model-only", "text": text, "disk": list(disk), "revision": revision,
            "dirty": encoded(text) != disk, "trace": trace}


def run_properties(seed=1901, max_cases=16, time_budget_ms=1000):
    _uint(seed)
    if type(max_cases) is not int or not 1 <= max_cases <= MAX_CASES or type(time_budget_ms) is not int or not 1 <= time_budget_ms <= MAX_BUDGET_MS:
        raise ValueError("property_budget")
    rng = random.Random(seed); traces = []; rejections = {}
    deadline = time.monotonic() + time_budget_ms / 1000
    reasons = ("revision_conflict", "invalid_utf16", "quota", "invalid_schema")
    for index in range(max_cases):
        if time.monotonic() >= deadline:
            return {"state": "timeout", "actual_cases": len(traces), "seed": seed, "traces": traces, "rejections": rejections}
        operation = {"op": "agent_edit", "expected_revision": 1, "start_utf16": 0,
                     "length_utf16": 0, "replacement": rng.choice(("ő", "😀", "é", "\ufeff"))}
        reason = reasons[index % 4]
        if reason == "revision_conflict": operation["expected_revision"] = 0
        elif reason == "invalid_utf16": operation["start_utf16"] = 1
        elif reason == "quota": operation["replacement"] = "x" * MAX_BYTES
        else: operation["extra"] = 1
        result = run_model_session("😀ár\n".encode(), [operation])
        if result["trace"][0]["status"] != reason or result["revision"] != 1 or result["text"] != "😀ár\n" or result["dirty"]:
            return {"state": "fail", "actual_cases": index + 1, "seed": seed, "minimal_reproducer": {"operation": operation}}
        rejections[reason] = rejections.get(reason, 0) + 1
        traces.append({"case_index": index, "rejection": reason, "trace": result["trace"]})
    return {"state": "pass", "actual_cases": len(traces), "seed": seed, "traces": traces, "rejections": rejections}


def negative_partial_edit_model(operations):
    """Deliberately broken own model, used only to exercise the shrinker."""
    files = {"a": "old", "b": "old"}
    for op in operations:
        if op == "partial_edit":
            files["a"] = "new"  # Deliberate negative control: b was not changed.
    return files not in ({"a": "old", "b": "old"}, {"a": "new", "b": "new"})


def minimize_reproducer(operations, reproduces, seed=1901, max_trials=32, time_budget_ms=500):
    _uint(seed)
    if not 1 <= len(operations) <= MAX_CASES or not 1 <= max_trials <= 32 or not 1 <= time_budget_ms <= MAX_BUDGET_MS:
        raise ValueError("reproducer_budget")
    deadline = time.monotonic() + time_budget_ms / 1000; trials = 0
    if not reproduces(operations):
        raise ValueError("not_reproducible")
    reduced = list(operations); index = 0
    while index < len(reduced) and trials < max_trials and time.monotonic() < deadline:
        candidate = reduced[:index] + reduced[index + 1:]; trials += 1
        if reproduces(candidate): reduced = candidate
        else: index += 1
    return {"seed": seed, "operations": reduced, "trials": trials,
            "bounded": True, "minimality": "single-deletion" if index == len(reduced) else "budget-limited"}


def source_identity():
    paths = [p for p in HERE.rglob("*") if p.is_file() and "__pycache__" not in p.parts and p.suffix != ".pyc"]
    paths += [ROOT / p for p in ("crates/latexide-contracts/src/quality_corpus.rs", "tests/contracts/v1/p19.json",
                                "integration/registry/ports.v1.json", "integration/registry/owners.v1.json")]
    records = [{"path": p.relative_to(ROOT).as_posix(), "sha256": _sha(p.read_bytes()),
                "size": p.stat().st_size, "mode": oct(p.stat().st_mode & 0o777)} for p in sorted(paths)]
    return {"sha256": _sha(_bytes(records)), "files": records}


def _artifact(path):
    return {"path": str(path), "sha256": _sha(path.read_bytes()), "size": path.stat().st_size,
            "mode": oct(path.stat().st_mode & 0o777)}


def _scratch():
    value = os.environ.get("P19_SCRATCH")
    if not value or not Path(value).is_absolute() or not Path(value).is_dir():
        raise ValueError("missing_private_scratch")
    return Path(value).resolve()


def _validate_case(case):
    _shape(case, ("schema", "case_id", "profile", "point", "acknowledged_revision", "seed", "mode", "timeout_ms"))
    if type(case["schema"]) is not int or case["schema"] != 1 or not isinstance(case["case_id"], str) or not re.fullmatch(r"[a-z0-9-]{1,64}", case["case_id"]):
        raise ValueError("invalid_case")
    if case["profile"] not in ("singlefile", "multifile-hu") or case["mode"] not in MODES:
        raise ValueError("invalid_case")
    expected_fault_outcome(case["point"], case["acknowledged_revision"])
    if not 1 <= case["acknowledged_revision"] < 2**64 - 1:
        raise ValueError("invalid_case")
    _uint(case["seed"]); _uint(case["timeout_ms"], 2000)
    if case["timeout_ms"] < 50:
        raise ValueError("invalid_case")


def _variants(profile):
    before = {item["path"]: item["raw_bytes"] if "raw_bytes" in item else list((HERE / "corpus" / profile["id"] / item["path"]).read_bytes()) for item in profile["files"]}
    after = {name: list(bytes(raw) + (b"% P19 synthetic agent\r\n" if b"\r\n" in bytes(raw) else b"% P19 synthetic agent\n")) if name.endswith(".tex") else raw for name, raw in before.items()}
    return before, after


def _disk(project, expected_names):
    actual = {p.relative_to(project).as_posix() for p in project.rglob("*") if p.is_file()}
    if actual != set(expected_names):
        raise ValueError("disk_file_set")
    if any(p.is_symlink() for p in project.rglob("*")):
        raise ValueError("disk_symlink")
    return {name: list((project / name).read_bytes()) for name in expected_names}


def _invoke(executable, request, recover, directory, timeout):
    argv = [sys.executable, "-I", "-B", str(executable), "--request", str(request)] + (["--recover"] if recover else [])
    started = time.monotonic(); phase = "recovery" if recover else "fault"
    try:
        process = subprocess.run(argv, cwd=directory, stdin=subprocess.DEVNULL, capture_output=True,
                                 env={"PATH": "/usr/bin:/bin", "TMPDIR": str(directory), "PYTHONUTF8": "1"},
                                 timeout=max(0.001, timeout))
        raw, errors, code, timed = process.stdout, process.stderr, process.returncode, False
    except subprocess.TimeoutExpired as error:
        # subprocess.run kills and reaps only the child it created. The helper
        # never starts another process; this is not a user-process kill.
        raw, errors, code, timed = error.stdout or b"", error.stderr or b"", None, True
    (directory / (phase + ".stdout.jsonl")).write_bytes(raw)
    (directory / (phase + ".stderr.log")).write_bytes(errors)
    return {"phase": phase, "argv": argv, "exit": code, "timeout": timed,
            "duration_ms": round((time.monotonic() - started) * 1000)}, raw


def run_fault_case(case: dict, executable: Path) -> dict:
    """Run only the owned synthetic helper; refuse arbitrary executables.

    The helper self-exits at a checkpoint. No daemon, GUI, compiler, IPC
    endpoint, live project, kernel power loss or actual P01 service is used.
    """
    result = {"schema": 1, "scope": "synthetic-helper-only", "state": "fail", "passed": False,
              "reason": None, "events": [], "processes": [], "artifacts": [],
              "producer_counts": {"actual_cases": 0, "pass": 0, "fail": 0, "error": 0, "skip": 0,
                                  "ignored": 0, "filtered": 0, "blocked": 0, "unrun": 0}}
    directory = None

    def finish(state, reason):
        result.update(state=state, reason=reason, passed=state == "pass")
        counts = result["producer_counts"]
        counts["actual_cases"] = int(any(e.get("case_count") == 1 for e in result["events"]))
        counts["pass" if state == "pass" else "error" if state == "timeout" else state] = 1
        if state != "pass" and isinstance(case, dict): result["minimal_reproducer"] = case
        if directory:
            result["artifacts"] = [_artifact(p) for p in sorted(directory.rglob("*")) if p.is_file() and p.name != "result.json"]
            (directory / "result.json").write_bytes(_bytes(result) + b"\n")
        return result

    try:
        _validate_case(case)
    except (ValueError, TypeError, KeyError):
        return finish("fail", "invalid_case")
    executable = Path(executable)
    if not executable.is_file(): return finish("blocked", "missing_executable")
    if executable.is_symlink() or executable.resolve() != HERE / "synthetic_helper.py":
        return finish("blocked", "unregistered_provider")
    try:
        profiles = load_profiles()
        source = source_identity()
        binding = {"case_id": case["case_id"], "point": case["point"], "acknowledged_revision": case["acknowledged_revision"],
                   "nonce": os.urandom(16).hex(), "input_sha256": _sha(_bytes({"case": case, "profiles": profiles})),
                   "source_sha256": source["sha256"], "executable_sha256": _sha(executable.read_bytes()),
                   "interpreter_sha256": _sha(Path(sys.executable).resolve().read_bytes()), "producer": "p19-synthetic-v1"}
        result.update(binding=binding, sources=source)
        directory = Path(tempfile.mkdtemp(prefix="fault-", dir=_scratch() / "tmp"))
        result["run_directory"] = str(directory)
        before, after = _variants(next(p for p in profiles if p["id"] == case["profile"]))
        project = directory / "project"; project.mkdir()
        for name, raw in before.items():
            file = project / name; file.parent.mkdir(parents=True, exist_ok=True); file.write_bytes(bytes(raw))
        request = directory / "request.json"
        request.write_bytes(_bytes({"schema": 1, "case": case, "binding": binding, "before": before, "after": after}))
        deadline = time.monotonic() + case["timeout_ms"] / 1000
        process, raw = _invoke(executable, request, False, directory, deadline - time.monotonic())
        result["processes"].append(process)
        if process["timeout"]: return finish("timeout", "fault_timeout")
        if len(raw) > 65536: return finish("fail", "log_quota")
        if not raw: return finish("fail", "zero_case")
        try: pre = _json(raw)
        except ValueError: return finish("fail", "event_schema")
        result["events"].append(pre if isinstance(pre, dict) else {})
        disk_before_recovery = _disk(project, before)
        if process["exit"] == 77: return finish("skip", "producer_skip")
        if process["exit"] != 86: return finish("fail", "fault_exit")
        process, raw = _invoke(executable, request, True, directory, deadline - time.monotonic())
        result["processes"].append(process)
        if process["timeout"]: return finish("timeout", "recovery_timeout")
        if process["exit"] != 0: return finish("fail", "recovery_exit")
        if not raw: return finish("fail", "missing_event")
        if len(raw) > 65536: return finish("fail", "log_quota")
        try: post = _json(raw)
        except ValueError: return finish("fail", "event_schema")
        result["events"].append(post if isinstance(post, dict) else {})
        extra_keys = {"schema", "phase", "seq", "case_count", "partial_edit_allowed", "recovered_revision", "truncated_tail_rejected"}
        for seq, event in enumerate(result["events"], 1):
            if set(event) != set(binding) | extra_keys or type(event["schema"]) is not int or event["schema"] != 1 or type(event["case_count"]) is not int or event["case_count"] != 1:
                return finish("fail", "event_schema")
            if any(event.get(key) != value or type(event.get(key)) is not type(value) for key, value in binding.items()):
                return finish("fail", "event_binding")
            if type(event["seq"]) is not int or event["seq"] != seq or event["phase"] != ("pre_fault" if seq == 1 else "post_recovery"):
                return finish("fail", "event_binding")
            if type(event["truncated_tail_rejected"]) is not bool or (seq == 1 and event["recovered_revision"] is not None) or (seq == 2 and (type(event["recovered_revision"]) is not int or not 0 <= event["recovered_revision"] < 2**64)):
                return finish("fail", "event_schema")
            if event["partial_edit_allowed"] is not False:
                return finish("fail", "partial_edit_claim")
        disk = _disk(project, before)
        if disk not in (before, after): return finish("fail", "partial_disk_edit")
        if disk != disk_before_recovery: return finish("fail", "recovery_saved_without_request")
        expected_disk = after if case["point"] == "after_rename" else before
        if disk != expected_disk: return finish("fail", "unexpected_disk_variant")
        recovered = _json((directory / "recovered.json").read_bytes())
        _shape(recovered, ("schema", "revision", "files"))
        minimum = expected_fault_outcome(case["point"], case["acknowledged_revision"])["minimum_recovered_revision"]
        if type(recovered["revision"]) is not int or recovered["revision"] < minimum or recovered["revision"] != case["acknowledged_revision"] or post["recovered_revision"] != recovered["revision"]:
            return finish("fail", "recovered_revision")
        if type(recovered["schema"]) is not int or recovered["schema"] != 1 or not isinstance(recovered["files"], dict) or any(not isinstance(values, list) or any(type(v) is not int or not 0 <= v <= 255 for v in values) for values in recovered["files"].values()):
            return finish("fail", "recovered_schema")
        if recovered["files"] != after:
            return finish("fail", "partial_recovered_edit")
        if source_identity() != source or _sha(executable.read_bytes()) != binding["executable_sha256"] or _sha(Path(sys.executable).resolve().read_bytes()) != binding["interpreter_sha256"]:
            return finish("fail", "source_or_executable_changed")
        if case["point"] == "mid_frame":
            try: decode_frame((directory / "ipc.frame").read_bytes())
            except ValueError: pass
            else: return finish("fail", "truncated_frame_accepted")
        result["observed"] = {"recovered_revision": recovered["revision"], "disk_variant": "after" if disk == after else "before",
                              "disk_sha256": _sha(_bytes(disk)), "recovered_sha256": _sha(_bytes(recovered)),
                              "truncated_tail_rejected": post["truncated_tail_rejected"]}
        return finish("pass", "synthetic_oracles_satisfied")
    except (OSError, ValueError, TypeError, KeyError) as error:
        result["detail"] = type(error).__name__ + ": " + str(error)
        return finish("error", "harness_error")


def recheck_fault_result(result):
    try:
        directory = Path(result["run_directory"])
        if not directory.resolve().is_relative_to(_scratch()) or source_identity() != result["sources"]:
            return {"valid": False, "reason": "source_changed"}
        if result["sources"]["sha256"] != result["binding"]["source_sha256"]:
            return {"valid": False, "reason": "source_binding"}
        if _sha(Path(sys.executable).resolve().read_bytes()) != result["binding"]["interpreter_sha256"]:
            return {"valid": False, "reason": "executable_changed"}
        paths = sorted(directory.rglob("*"))
        if any(p.is_symlink() for p in paths):
            return {"valid": False, "reason": "artifact_changed"}
        actual = [_artifact(p) for p in paths if p.is_file() and p.name != "result.json"]
        if actual != result["artifacts"]:
            return {"valid": False, "reason": "artifact_changed"}
        for record in result["artifacts"]:
            path = Path(record["path"])
            if path.is_symlink() or not path.resolve().is_relative_to(directory.resolve()) or _artifact(path) != record:
                return {"valid": False, "reason": "artifact_changed"}
        if (directory / "result.json").read_bytes() != _bytes(result) + b"\n":
            return {"valid": False, "reason": "receipt_changed"}
        return {"valid": bool(result["artifacts"]), "reason": "bound" if result["artifacts"] else "missing_artifacts"}
    except (OSError, KeyError, ValueError):
        return {"valid": False, "reason": "missing_artifact"}


def run_suite(cases, executable, max_cases=16, time_budget_ms=3000):
    if type(max_cases) is not int or not 1 <= max_cases <= MAX_CASES or type(time_budget_ms) is not int or not 1 <= time_budget_ms <= MAX_BUDGET_MS or not isinstance(cases, list) or not 1 <= len(cases) <= max_cases:
        return {"state": "fail", "passed": False, "reason": "suite_budget_or_zero_cases", "actual_cases": 0, "results": []}
    deadline = time.monotonic() + time_budget_ms / 1000; results = []
    for case in cases:
        remaining = int((deadline - time.monotonic()) * 1000)
        if remaining < 50: break
        adjusted = {**case, "timeout_ms": min(case.get("timeout_ms", 1000), remaining)}
        results.append(run_fault_case(adjusted, executable))
    unrun = len(cases) - len(results)
    passed = not unrun and bool(results) and all(r["state"] == "pass" for r in results)
    return {"schema": 1, "scope": "synthetic-helper-only", "state": "pass" if passed else "fail", "passed": passed,
            "actual_cases": len(results), "pass": sum(r["passed"] for r in results),
            "fail": sum(r["state"] == "fail" for r in results), "error": sum(r["state"] == "error" for r in results),
            "skip": sum(r["state"] == "skip" for r in results), "timeout": sum(r["state"] == "timeout" for r in results),
            "blocked": sum(r["state"] == "blocked" for r in results), "unrun": unrun, "ignored": 0, "filtered": 0, "results": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    cases = [{"schema": 1, "case_id": profile + "-" + point.replace("_", "-"), "profile": profile,
              "point": point, "acknowledged_revision": 7, "seed": 1901, "mode": "normal", "timeout_ms": 1000}
             for profile in ("singlefile", "multifile-hu") for point in POINTS]
    result = run_suite(cases, HERE / "synthetic_helper.py")
    result["properties"] = run_properties()
    result["passed"] = result["passed"] and result["properties"]["state"] == "pass"
    result["state"] = "pass" if result["passed"] else "fail"
    result["unqualified"] = ["P01 recovery", "P03 IPC/runtime", "TeX/compiler recipes", "power-loss/disk-full", "long soak", "native macOS", "Windows11", "Linux"]
    args.report.write_bytes(_bytes(result) + b"\n")
    print(json.dumps({key: value for key, value in result.items() if key not in ("results", "properties")}, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
