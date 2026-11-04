"""P01 parent-owned crash cohorts. No child entry is a standalone test.

The driver is only called by the trusted P00 step runner. Stored receipts and
manifest commands are data. Revalidation performs no process execution.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PureWindowsPath
import re
import secrets
import stat
import sys

# Filled from the explicitly frozen, reviewed P01 input by the candidate builder.
SOURCE_PINS = {'crates/latexide-core/tests/recovery_contract.rs': {'sha256': '969fa145f611a4017875c7ee0350421ee495da08d43138004521f2f9e3b51709', 'mode': 420, 'bytes': 15497}, 'crates/latexide-core/src/services/recovery/durable_tests.rs': {'sha256': '12d99f121a6df6c9573f159b7acafa0ee54617c5201698227b4515f1c64673e3', 'mode': 420, 'bytes': 31208}, 'crates/latexide-core/src/services/recovery/runtime_tests.rs': {'sha256': 'ef0f62ca18c3bc8431135504bb889f8fbd3dabc401e50b10f07edb1e95295208', 'mode': 420, 'bytes': 22326}}
SENTINELS = ("crates/latexide-core/src/services/recovery/durable_tests.rs",
             "crates/latexide-core/src/services/recovery/runtime_tests.rs")
PARENTS = {
    "durable": "durable_contract::durable_own_child_exit_matrix_preserves_last_caller_observed_ack",
    "runtime": "runtime_contract::runtime_own_process_exit_restores_committed_state_save_intent_and_undo",
}
CHILDREN = {"durable": "durable_contract::durable_crash_child",
            "runtime": "runtime_contract::runtime_crash_child"}
EXCLUDED = tuple(sorted([*PARENTS.values(), *CHILDREN.values()]))
APPEND = ("JournalWrite", "JournalPartiallyWritten", "JournalWritten", "JournalFlushed",
          "JournalSync", "JournalSynced", "JournalParentSync", "JournalParentSynced",
          "BeforeAck", "after_return")
CHECKPOINT = ("CheckpointWrite", "CheckpointPartiallyWritten", "CheckpointWritten",
              "CheckpointFlushed", "CheckpointSync", "CheckpointSynced", "CheckpointRename",
              "CheckpointRenamed", "CheckpointParentSync", "CheckpointParentSynced",
              "JournalWrite", "JournalPartiallyWritten", "JournalWritten", "JournalFlushed",
              "JournalSync", "JournalSynced", "JournalRename", "JournalRenamed",
              "JournalParentSync", "JournalParentSynced", "BeforeAck", "after_return")
RUNTIME = {
    "edit_before_publish": ("BeforePublish", 2),
    "edit_after_publish": ("Published", 2),
    "save_intent_published": ("Published", 3),
    "save_final_prepared": ("Prepared", 4),
    "undo_after_save": ("Published", 5),
}
OWN_ENV = ("P01_CHILD_ROOT", "P01_CHILD_MODE", "P01_CHILD_PHASE", "P01_CHILD_PROJECT",
           "P01_CHILD_STORAGE", "P01_CHILD_CASE", "P01_CHILD_NONCE", "P01_RUN_NONCE",
           "P01_DURABLE_SCRATCH", "P01_RUNTIME_SCRATCH")
P18_ENV = ("LATEXIDE_P18_PHASE2_ROOT", "LATEXIDE_P18_PHASE3_ROOT",
           "LATEXIDE_P18_TEX_PREFIX", "LATEXIDE_P18_TEX_RUN")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def same(a, b):
    return json.dumps(a, sort_keys=True, separators=(",", ":"), allow_nan=False) == json.dumps(
        b, sort_keys=True, separators=(",", ":"), allow_nan=False)


def uint(value, message, maximum=None):
    require(type(value) is int and value >= 0 and (maximum is None or value <= maximum), message)
    return value


def read(path, limit=1024 * 1024):
    def unique(pairs):
        result = {}
        for k, v in pairs:
            require(k not in result, "Duplicate P01 evidence field")
            result[k] = v
        return result
    with Path(path).open("rb") as f:
        raw = f.read(limit + 1)
    require(len(raw) <= limit, "P01 evidence quota")
    return json.loads(raw, object_pairs_hook=unique)


def text(path, limit=1024 * 1024):
    with Path(path).open("rb") as f:
        raw = f.read(limit + 1)
    require(len(raw) <= limit, "P01 raw log quota")
    return raw.decode("utf-8", errors="strict")


def _mode(path):
    """Actual filesystem permission observation; synthetic tests may model this boundary."""
    return stat.S_IMODE(Path(path).stat().st_mode)


def binding(path):
    path = Path(path)
    s = path.lstat()
    require(stat.S_ISREG(s.st_mode) and not path.is_symlink(), "P01 regular file required")
    with path.open("rb") as f:
        sha = hashlib.file_digest(f, "sha256").hexdigest()
    return {"path": str(path.resolve()), "sha256": sha, "bytes": s.st_size,
            "mode": _mode(path), "device": s.st_dev, "inode": s.st_ino,
            "uid": s.st_uid, "kind": "file"}


def scratch_binding(scratch):
    s = scratch.lstat()
    require(stat.S_ISDIR(s.st_mode) and not scratch.is_symlink(), "P01 scratch directory type")
    return {"path": str(scratch.resolve()), "device": s.st_dev, "inode": s.st_ino,
            "uid": s.st_uid, "mode": _mode(scratch),
            "marker": binding(scratch / "COHORT")}


def private_path(path, parent, directory=False):
    path, owner = Path(path), Path(parent)
    require(path.is_absolute() and owner.is_absolute() and ".." not in path.parts
            and ".." not in owner.parts, "Absolute lexical owned P01 output required")
    require(path.is_relative_to(owner), "P01 output is not lexically under its owner")
    require(path.resolve(strict=True).is_relative_to(owner.resolve(strict=True)), "P01 output escaped owned scratch")
    require(not owner.is_symlink(), "P01 owner symlink")
    current = owner
    for part in path.relative_to(owner).parts:
        current /= part
        require(not current.is_symlink(), "P01 output symlink")
    require(path.is_dir() if directory else path.is_file(), "Missing P01 output")
    return path


def catalog(root):
    root = Path(root)
    present = [p for p in SENTINELS if (root / p).exists()]
    if not present:
        return None  # The current committed main has only the read-only first slice.
    require(len(present) == len(SENTINELS) and bool(SOURCE_PINS), "Partial P01 cohort source")
    observed = {}
    for name, expected in SOURCE_PINS.items():
        row = binding(root / name)
        require(same({k: row[k] for k in ("sha256", "bytes", "mode")}, expected),
                "Unreviewed P01 child/parent source contract")
        observed[name] = row
    return observed


def skipped_arguments():
    return [v for name in EXCLUDED for v in ("--skip", name)]


def prepare_record(record, expected_argv, run_id, source_sha256, tool):
    """Detach the actual step record; types/status are checked again on readback."""
    require(isinstance(run_id, str) and re.fullmatch("[0-9a-f]{32}", run_id) is not None, "P01 step nonce")
    require(isinstance(source_sha256, str) and re.fullmatch("[0-9a-f]{64}", source_sha256) is not None, "P01 source digest")
    require(same(record.get("command"), expected_argv), "P01 parent driver command changed")
    require(record.get("status") == "passed" and type(record.get("exit_code")) is int
            and record["exit_code"] == 0 and record.get("timed_out") is False,
            "P01 parent driver did not pass")
    require(record.get("name") == "rust-workspace", "P01 parent driver identity")
    require(record.get("log", {}).get("kind") == "command_log", "P01 parent log kind")
    require(binding(record["log"]["path"])["sha256"] == record["log"]["sha256"],
            "P01 parent driver log changed")
    require(same(binding(tool["path"]), tool), "P01 build tool changed")
    context = {"name": record["name"], "command": record["command"],
            "exit_code": record["exit_code"], "timed_out": record["timed_out"],
            "status": record["status"], "log": binding(record["log"]["path"]),
            "verification_run_id": run_id, "source_sha256": source_sha256, "tool": tool}
    if "p18_environment" in record:
        context["p18_environment"] = record["p18_environment"]
    return context


def build_images(root, context, target):
    """Read Cargo messages captured by this exact successful step, never glob old bins."""
    require(same(binding(context["log"]["path"]), context["log"]), "Stale P01 Rust log")
    raw = text(context["log"]["path"], 16 * 1024 * 1024)
    images = []
    for line in raw.splitlines():
        try:
            item = json.loads(line)
        except ValueError:
            continue
        if not isinstance(item, dict) or item.get("reason") != "compiler-artifact":
            continue
        if not item.get("executable") or item.get("profile", {}).get("test") is not True:
            continue
        binary = Path(item["executable"])
        require(type(item.get("fresh")) is bool, "Cargo fresh flag type")
        require(binary.resolve(strict=True).is_relative_to(Path(target).resolve(strict=True)),
                "P01 binary outside actual target")
        b = binding(binary)
        require(b["mode"] & 0o111, "P01 test image is not executable")
        images.append({"target": item["target"]["name"], "src_path": item["target"]["src_path"],
                       "fresh": item.get("fresh"), "binary": b})
    require(images and len({x["binary"]["path"] for x in images}) == len(images),
            "Missing or duplicate captured Cargo test images")
    owned = [x for x in images if x["target"] == "recovery_contract"
             and Path(x["src_path"]).resolve() == (Path(root) / "crates/latexide-core/tests/recovery_contract.rs").resolve()]
    require(len(owned) == 1 and owned[0]["fresh"] is False, "No fresh current P01 test artifact")
    return images, owned[0], raw


def parse_discovery(raw):
    ids = []
    for line in raw.splitlines():
        if not line.strip():
            continue
        m = re.fullmatch(r"([A-Za-z_][A-Za-z_0-9:]*)\: test", line)
        if m:
            ids.append(m[1])
        else:
            footer = re.fullmatch(r"(\d+) tests?, (\d+) benchmarks?", line)
            require(footer is not None and int(footer[1]) == len(ids) and int(footer[2]) == 0,
                    "Unknown P01 discovery output")
    require(len(ids) == len(set(ids)), "Duplicate P01 discovery ID")
    return ids


def libtest_observation(raw, allow_empty=False):
    """Complete named outcomes partitioned by actual libtest summaries.

    A summary never completes an unfinished named test. Unknown/interleaved
    terminal formats fail closed; this is not a doctest/bench format fallback.
    """
    require(type(raw) is str and len(raw.encode("utf-8")) <= 16 * 1024 * 1024,
            "Libtest raw input quota/type")
    events, partitions, current = [], [], []
    pending = None
    declared = None
    summary_pattern = (r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
                       r"(\d+) measured; (\d+) filtered out;(?: finished in [0-9.]+s)?")
    def terminal(value):
        if value in ("ok", "FAILED"): return value
        if value == "ignored" or value.startswith("ignored, "): return "ignored"
        return None
    for line in raw.splitlines():
        if line.startswith("test result:"):
            found = re.fullmatch(summary_pattern, line)
            require(found is not None and pending is None, "Missing named terminal outcome or malformed libtest summary")
            status, passed, failed, ignored, measured, filtered = found.groups()
            counts = Counter(outcome for name, outcome in current)
            require(declared is None or declared == len(current), "Running header disagrees with named methods")
            require(int(measured) == 0 and (counts["ok"], counts["FAILED"], counts["ignored"])
                    == (int(passed), int(failed), int(ignored)), "Named outcomes disagree with libtest counts")
            require(len({name for name, outcome in current}) == len(current), "Duplicate named method in one partition")
            require((status == "ok") == (int(failed) == 0), "Libtest result status disagrees with failures")
            partitions.append({"status": status, "passed": int(passed), "failed": int(failed),
                               "ignored": int(ignored), "filtered": int(filtered), "events": list(current)})
            current = []; declared = None
        elif re.fullmatch(r"running (\d+) tests?", line):
            require(pending is None and not current and declared is None, "Unexpected running header in a test partition")
            declared = int(re.fullmatch(r"running (\d+) tests?", line).group(1))
        elif line.startswith("test "):
            found = re.fullmatch(r"test ([A-Za-z_][A-Za-z_0-9:]*) \.\.\.(?: (.*))?", line)
            require(found is not None and pending is None, "Unknown or unfinished named libtest method")
            name, tail = found.groups()
            state = terminal((tail or "").strip())
            if state is None: pending = name
            else: current.append((name, state)); events.append((name, state))
        elif terminal(line.strip()) is not None:
            require(pending is not None, "Unowned libtest terminal outcome")
            item = (pending, terminal(line.strip())); current.append(item); events.append(item); pending = None
    require(pending is None and declared is None and not current and partitions, "Missing libtest terminal outcome or summary")
    require(allow_empty or bool(events), "Zero named libtest methods")
    return {"events": events, "partitions": partitions}


def phase_binding(record, name, image, context, scratch, nonce):
    require(record.get("name") == name, "P01 phase identity changed")
    require(record.get("log", {}).get("path") == str(Path(scratch) / (name + ".log")),
            "P01 phase log partition changed")
    require(same(record.get("context_binding"), {"phase": name,
        "verification_run_id": context["verification_run_id"], "source_sha256": context["source_sha256"],
        "binary": image["binary"], "driver_nonce": nonce}), "P01 phase binary/source/run binding changed")


def normal_command(image, ids):
    return [image["binary"]["path"], "--include-ignored", "--nocapture", "--test-threads=1",
            *[value for name in EXCLUDED if name in ids for value in ("--skip", name)]]


def _p18_module():
    path = Path(__file__).with_name("p18_ci.py")
    spec = importlib.util.spec_from_file_location("p01_p18_contract", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _directory_binding(path, private=False, *, owner=None):
    path = Path(path)
    # System aliases above the declared owner are not fixture symlinks.
    private_path(path, path if owner is None else owner, directory=True)
    s = path.lstat()
    require(stat.S_ISDIR(s.st_mode), "P18 directory object required")
    require(not private or _mode(path) == 0o700, "P18 namespace must be private")
    return {"path": str(path), "device": s.st_dev, "inode": s.st_ino,
            "uid": s.st_uid, "mode": _mode(path)}


def _p18_outputs(env, raw, ids, require_markers=True):
    """Seal the actual marker-selected runs; PDF semantics remain the P18 gate."""
    outputs = {}
    for phase, method in zip((2, 3), _p18_module().REQUIRED[2:]):
        root = Path(env[f"LATEXIDE_P18_PHASE{phase}_ROOT"])
        markers = re.findall(r"P18_PHASE" + str(phase) + r"_TEX_ARTIFACTS=([^\r\n]+)", raw)
        if method not in ids:
            require(not markers, "Foreign P18 phase marker")
            continue
        run = root / "artifacts" / env["LATEXIDE_P18_TEX_RUN"]
        private_path(run, root, directory=True)
        require(markers in ([str(run)], [str(run.resolve(strict=True))]) or (not require_markers and not markers),
                "Missing/stale P18 invocation proof")
        directories, files = [_directory_binding(run, owner=root)], []
        for path in sorted(run.rglob("*")):
            require(not path.is_symlink(), "P18 proof symlink")
            if path.is_dir():
                directories.append(_directory_binding(path, owner=run))
            else:
                files.append(binding(private_path(path, run)))
        for role in ("control", "protected"):
            directory = role if phase == 2 else role + "-generation"
            pdf = private_path(run / directory / "output/main.pdf", run)
            require(pdf.stat().st_size > 0, "Missing P18 compiler PDF bytes")
        if phase == 3:
            binding(private_path(run / "generation-publication.receipt", run))
        outputs[str(phase)] = {"directories": directories, "files": files}
    return outputs


def _p18_inherited(env, raw):
    selected = {key: env[key] for key in P18_ENV if key in env}
    if not selected:
        require(not re.search(r"P18_PHASE[23]_TEX_ARTIFACTS=", raw), "Missing P18 Cargo namespace proof")
        return None
    require(set(selected) == set(P18_ENV) and all(type(v) is str for v in selected.values()),
            "Partial P18 invocation environment")
    require(re.fullmatch(r"[a-zA-Z0-9-]+", selected["LATEXIDE_P18_TEX_RUN"]), "Invalid P18 run label")
    p18 = _p18_module()
    ids = p18.validate_workspace_log(raw)
    roots = []
    for phase in (2, 3):
        root = Path(selected[f"LATEXIDE_P18_PHASE{phase}_ROOT"])
        roots.extend((_directory_binding(root, True), _directory_binding(root / "artifacts", True, owner=root)))
    first, second = (Path(selected[key]).resolve(strict=True) for key in P18_ENV[:2])
    require(not first.is_relative_to(second) and not second.is_relative_to(first), "P18 phase roots overlap")
    prefix = Path(selected["LATEXIDE_P18_TEX_PREFIX"])
    private_path(prefix, prefix.parent, directory=True)
    receipt = binding(prefix.parent / "provision.json")
    require(p18.validate_toolchain(prefix) == receipt["sha256"], "P18 toolkit/provision changed")
    return {"environment": selected, "directories": roots, "provision": receipt,
            "validator": binding(Path(p18.__file__)),
            "outputs": _p18_outputs(selected, raw, ids, require_markers=False)}


def _p18_namespace(scratch, name, image, context, nonce, command, inherited, create=False):
    directory = Path(scratch) / ("p18-" + name)
    identity = {"phase": name, "verification_run_id": context["verification_run_id"],
                "source_sha256": context["source_sha256"], "binary": image["binary"],
                "driver_nonce": nonce, "command": command,
                "inherited_sha256": hashlib.sha256(json.dumps(inherited, sort_keys=True,
                    separators=(",", ":")).encode()).hexdigest()}
    digest = hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    env = {"LATEXIDE_P18_TEX_PREFIX": inherited["environment"]["LATEXIDE_P18_TEX_PREFIX"],
           "LATEXIDE_P18_TEX_RUN": "p01-" + digest,
           **{f"LATEXIDE_P18_PHASE{phase}_ROOT": str(directory / ("phase" + str(phase))) for phase in (2, 3)}}
    expected = {"schema": 1, "identity": identity, "environment": env}
    marker = directory / "NAMESPACE.json"
    if create:
        directory.mkdir(mode=0o700)  # never reuse, remove, or overwrite an invocation
        for phase in (2, 3):
            root = Path(env[f"LATEXIDE_P18_PHASE{phase}_ROOT"])
            root.mkdir(mode=0o700)
            for child in ("artifacts", "fixtures"):
                (root / child).mkdir(mode=0o700)
        with marker.open("x", encoding="utf-8") as stream:
            json.dump(expected, stream, sort_keys=True, indent=2)
        marker.chmod(0o600)
    private_path(directory, scratch, directory=True)
    require(_mode(marker) == 0o600 and not marker.is_symlink() and same(read(marker), expected),
            "P18 invocation source/binary/nonce/argv/proof changed")
    directories = [_directory_binding(directory, True, owner=scratch)]
    for phase in (2, 3):
        root = Path(env[f"LATEXIDE_P18_PHASE{phase}_ROOT"])
        directories.extend(_directory_binding(path, True, owner=directory) for path in (root, root / "artifacts", root / "fixtures"))
    return {"environment": env, "metadata": binding(marker), "directories": directories}


def normal_observation(images, discoveries, records, scratch, nonce, context, p18=None):
    require(type(records) is list and len(records) == len(images), "Missing normal image partition")
    identities, observations = [], []
    for index, (image, discovery, record) in enumerate(zip(images, discoveries, records)):
        name = "normal-" + str(index)
        phase_binding(record, name, image, context, scratch, nonce)
        require(same(record.get("owned_environment"), owned_environment(scratch, nonce)), "Normal nonce/environment changed")
        discovered = parse_discovery(text(discovery["all"]["log"]["path"]))
        namespace = None
        if p18 is not None:
            namespace = _p18_namespace(scratch, name, image, context, nonce,
                                       normal_command(image, discovered), p18)
            namespace["outputs"] = _p18_outputs(namespace["environment"], text(record["log"]["path"]), discovered)
            require(same(record.get("p18_namespace"), namespace), "P18 normal namespace/proof changed")
            require(same(read(Path(scratch) / (name + ".json")), record), "P18 normal execution record changed")
        else:
            require("p18_namespace" not in record, "Unbound P18 normal namespace")
        wanted = sorted(set(discovered) - set(EXCLUDED))
        observed = libtest_observation(command_record(record, normal_command(image, discovered), scratch), allow_empty=True)
        require(len(observed["partitions"]) == 1, "Normal image has multiple or reused partitions")
        partition = observed["partitions"][0]
        require(partition["status"] == "ok" and partition["failed"] == partition["ignored"] == 0
                and partition["filtered"] == len(set(discovered) & set(EXCLUDED)), "Normal image outcomes/filters differ")
        require(sorted(name for name, status in observed["events"]) == wanted
                and all(status == "ok" for name, status in observed["events"]), "Normal raw method identity differs from discovery")
        for method in wanted:
            identities.append({"binary_path": image["binary"]["path"], "binary_sha256": image["binary"]["sha256"],
                "method_id": method, "source_sha256": context["source_sha256"],
                "verification_run_id": context["verification_run_id"]})
        observations.append({"binary": image["binary"], "method_ids": wanted, "log": binding(record["log"]["path"])})
        if namespace is not None:
            observations[-1]["p18_namespace"] = namespace
    require(identities and len({(row["binary_path"], row["method_id"]) for row in identities}) == len(identities),
            "Zero or duplicate compound normal identity")
    return identities, observations


def command_record(record, expected, scratch, timeout=10):
    require(same(record.get("command"), expected), "P01 command/argc mismatch")
    require(record.get("status") == "passed" and type(record.get("exit_code")) is int
            and record["exit_code"] == 0 and record.get("timed_out") is False,
            "P01 discovery/parent process did not pass")
    uint(record.get("duration_ms"), "P01 wait bound/type", timeout * 1000)
    require(type(record.get("argc")) is int and record["argc"] == len(expected), "P01 argc binding")
    require(type(record.get("requested_timeout")) is int and record["requested_timeout"] == timeout,
            "P01 finite own wait required")
    p = private_path(record["log"]["path"], scratch)
    require(binding(p)["sha256"] == record["log"]["sha256"], "Stale P01 process log")
    return text(p)


def require_parent_command(actual, expected):
    require(same(actual, expected), "Wrong P01 parent/child relationship")


def owned_environment(scratch, nonce):
    return {"P01_RUN_NONCE": nonce, "P01_DURABLE_SCRATCH": str(scratch / "durable"),
            "P01_RUNTIME_SCRATCH": str(scratch / "runtime"), "TMPDIR": str(scratch / "tmp")}


def discovery_observation(images, image, records, scratch, nonce, context):
    require(isinstance(records, list) and len(records) == len(images), "Missing P01 discovery")
    exclusions = []
    owned_ids = None
    discovered = 0
    for index, (img, pair) in enumerate(zip(images, records)):
        require(set(pair) == {"all", "ignored"}, "Unknown discovery role")
        for record in pair.values():
            require(same(record.get("owned_environment"), owned_environment(scratch, nonce)),
                    "Discovery nonce/environment binding changed")
        phase_binding(pair["all"], "discovery-all-" + str(index), img, context, scratch, nonce)
        phase_binding(pair["ignored"], "discovery-ignored-" + str(index), img, context, scratch, nonce)
        binary = img["binary"]["path"]
        ids = parse_discovery(command_record(pair["all"], [binary, "--list", "--format", "terse"], scratch))
        ignored = parse_discovery(command_record(pair["ignored"], [binary, "--ignored", "--list", "--format", "terse"], scratch))
        require(set(ignored).issubset(ids), "Ignored discovery differs from all discovery")
        matches = [(name, skip) for name in ids for skip in EXCLUDED if skip in name]
        if img == image:
            require(ids and all(name in ids for name in EXCLUDED)
                    and set(ignored) == set(CHILDREN.values()), "Unknown/missing P01 helper or parent discovery")
            owned_ids = ids
        exclusions.extend((binary, name, skip) for name, skip in matches)
        discovered += len(ids)
    require(len(exclusions) == 4 and set(exclusions) == {(image["binary"]["path"], n, n) for n in EXCLUDED},
            "Skip collision/unknown P01 exclusion")
    return owned_ids, discovered


def child_entry_evidence(raw, child_id):
    # P01_CHILD_RESULT_GUARD: an exit73 helper never completes libtest successfully.
    raw = raw.replace("\r\n", "\n")
    if raw == "\nrunning 1 test\n":
        return False  # Source/parent-bound role, not a raw ID observation.
    incomplete = r"\n?running 1 test\ntest " + re.escape(child_id) + r" \.\.\.[ \t\r\n]*"
    require(re.fullmatch(incomplete, raw) is not None,
            "Child PASS or unknown/missing entry transcript is not parent coverage")
    return True


def parent_observation(kind, record, image, scratch, driver_nonce, discovered, context, timeout=10):
    parent_id, child_id = PARENTS[kind], CHILDREN[kind]
    expected = [image["binary"]["path"], parent_id, "--exact", "--nocapture", "--test-threads=1"]
    require_parent_command(record.get("command"), expected)
    phase_binding(record, "parent-" + kind, image, context, scratch, driver_nonce)
    require(same(record.get("owned_environment"), owned_environment(scratch, driver_nonce)),
            "Parent nonce/environment binding changed")
    raw = command_record(record, expected, scratch, timeout)
    # P01_PARENT_TERMINAL_GUARD: counters never complete a named parent.
    parsed = libtest_observation(raw)
    require(parsed["events"] == [(parent_id, "ok")] and len(parsed["partitions"]) == 1,
            "Exact parent did not have an actual terminal PASS")
    require(re.findall(r"^test (\S+) \.\.\.", raw, re.M) == [parent_id],
            "P01 parent selection is missing/duplicated/wrong")
    outcomes = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", raw)
    require(len(outcomes) == 1 and outcomes[0][0:5] == ("ok", "1", "0", "0", "0"),
            "P01 exact parent result missing or failed")
    require(outcomes[0][5] == str(discovered - 1), "P01 parent result/discovery mismatch")
    if kind == "durable":
        marker = re.findall(r"P01_CHILD_CASES=32 PASS=32 FAIL=0 ERROR=0 SKIP=0 nonce=([0-9a-f]{64}) evidence=([^\r\n]+)", raw)
    else:
        marker = re.findall(r"P01_RUNTIME_CHILD_CASES=5 PASS=5 FAIL=0 ERROR=0 nonce=([0-9a-f]{64}) evidence=([^\r\n]+)", raw)
    require(len(marker) == 1, "Missing/duplicate P01 parent case summary")
    nonce, summary = marker[0]
    require(kind != "durable" or nonce == driver_nonce, "Durable child nonce differs from own driver")
    table = Path(summary) / "32-process-boundary-cases.txt" if kind == "durable" else Path(summary)
    private_path(table, scratch)
    table_raw = text(table)
    cases = []
    if kind == "durable":
        expected_keys = [(mode, phase) for mode, phases in (("append", APPEND), ("checkpoint", CHECKPOINT)) for phase in phases]
        pattern = (r"mode=(\w+) phase=(\w+) exit=(\d+) observed_sequence=(\d+) recovered_sequence=(\d+) "
                   r"strict_rejected=(true|false) duration_ms=(\d+) binary_sha256=([0-9a-f]{64}) nonce=([0-9a-f]{64}) fixture=(.+)")
        for line in table_raw.splitlines():
            m = re.fullmatch(pattern, line)
            require(m is not None, "Malformed durable child row")
            mode, phase, code, observed, recovered, rejected, elapsed, binary, case_nonce, place = m.groups()
            key = (mode, phase)
            require(key in expected_keys and int(code) == 73 and int(elapsed) < 10000,
                    "Unknown durable phase/exit/wait")
            require(binary == image["binary"]["sha256"] and case_nonce == nonce, "Durable binary/nonce mismatch")
            want = 8 if key == ("append", "after_return") else 7
            require(int(observed) == int(recovered) == want, "Durable ack/restore relationship")
            place = private_path(place, scratch, directory=True)
            require(_mode(place) == 0o700, "Nonprivate durable fixture")
            child = private_path(place / "child.log", scratch)
            child_text = text(child)
            raw_id_observed = child_entry_evidence(child_text, child_id)
            trace = private_path(place / "phases.log", scratch)
            stream = text(trace).splitlines()
            phases = APPEND if mode == "append" else CHECKPOINT
            want_trace = list(phases[:-1]) if phase == "after_return" else list(phases[:phases.index(phase) + 1])
            require(stream == want_trace, "Durable phase transcript differs")
            ack = private_path(place / "observed-r7.receipt", scratch)
            after = place / "observed-after-return.receipt"
            require(after.exists() == (phase == "after_return"), "Unexpected after-return ack")
            cases.append({"key": [mode, phase], "exit": 73, "duration_ms": int(elapsed),
                          "binary_sha256": binary, "nonce": nonce, "fixture": str(place),
                          "observed_sequence": int(observed), "recovered_sequence": int(recovered),
                          "strict_rejected": rejected == "true",
                          "raw_child_ID_observed": raw_id_observed,
                          "child_role_evidence": "libtest-prefix" if raw_id_observed else "source-bound parent argv and fixture",
                          "files": [binding(child), binding(trace), binding(ack)] + ([binding(after)] if after.exists() else [])})
        require([tuple(x["key"]) for x in cases] == expected_keys, "Missing/duplicate/reordered durable cases")
    else:
        pattern = r"case=(\w+) exit=(\d+) duration_ms=(\d+) binary_sha256=([0-9a-f]{64}) nonce=([0-9a-f]{64}) project=(.+)"
        for line in table_raw.splitlines():
            m = re.fullmatch(pattern, line)
            require(m is not None, "Malformed runtime child row")
            case, code, elapsed, binary, case_nonce, project = m.groups()
            require(case in RUNTIME and int(code) == 73 and int(elapsed) < 10000, "Unknown runtime case/exit/wait")
            require(binary == image["binary"]["sha256"] and case_nonce == nonce, "Runtime binary/nonce mismatch")
            project = private_path(project, scratch, directory=True)
            require(project.name == "project" and _mode(project.parent) == 0o700,
                    "Runtime private project owner missing")
            storage = private_path(project.parent / "recovery", scratch, directory=True)
            child = private_path(project.parent / "child.log", scratch)
            child_text = text(child)
            raw_id_observed = child_entry_evidence(child_text, child_id)
            phase, occurrence = RUNTIME[case]
            cases.append({"key": case, "exit": 73, "duration_ms": int(elapsed),
                          "binary_sha256": binary, "nonce": nonce, "project": str(project),
                          "storage": str(storage), "source_defined_phase": phase,
                          "source_defined_occurrence": occurrence, "files": [binding(child)]})
            cases[-1].update(raw_child_ID_observed=raw_id_observed,
                             child_role_evidence="libtest-prefix" if raw_id_observed else "source-bound parent argv and fixture")
        require([x["key"] for x in cases] == list(RUNTIME), "Missing/duplicate/reordered runtime cases")
    return {"parent_id": parent_id, "child_id": child_id, "parent_methods": 1,
            "helper_entries": 1, "child_passes": 0, "subcases": len(cases), "nonce": nonce,
            "nonce_origin": "driver" if kind == "durable" else "Rust-parent-generated",
            "source_defined_child_argv": ["--ignored", "--exact", child_id, "--nocapture"],
            "direct_child_argv_attestation": False,
            "phase_evidence": "retained hook transcript" if kind == "durable" else "source-defined ExitPoint + parent assertions",
            "table": binding(table), "parent_log": binding(record["log"]["path"]), "cases": cases}


def _normal_final(proof):
    """Rebind sealed observations after other source/artifact/parent reads."""
    for row in proof["normal_observations"]:
        require(same(row["log"], binding(row["log"]["path"])), "Late normal raw observation changed")
        require(same(row["binary"], binding(row["binary"]["path"])), "Late normal image changed")
        namespace = row.get("p18_namespace")
        if namespace:
            require(same(namespace["outputs"], _p18_outputs(namespace["environment"],
                text(row["log"]["path"]), row["method_ids"])), "Late P18 output inventory changed")
            require(same(namespace["metadata"], binding(namespace["metadata"]["path"])), "Late P18 namespace proof changed")
            for directory in namespace["directories"]:
                require(same(directory, _directory_binding(directory["path"], True,
                    owner=Path(namespace["metadata"]["path"]).parent)), "Late P18 namespace directory changed")
            for output in namespace["outputs"].values():
                for directory in output["directories"]:
                    require(same(directory, _directory_binding(directory["path"],
                        owner=output["directories"][0]["path"])), "Late P18 output directory changed")
                for file in output["files"]:
                    require(same(file, binding(file["path"])), "Late P18 output proof changed")


def rebuild(root, context, proof, target):
    require(isinstance(proof, dict) and set(proof) == {
        "schema", "suite", "context", "scratch", "driver_nonce", "source_before",
        "source_after", "adapter", "images", "discovery", "normal", "normal_observations", "parents", "observations", "scratch_binding", "p18"},
        "Unknown/missing P01 proof field")
    require(type(proof.get("schema")) is int and proof["schema"] == 3
            and proof.get("suite") == "p01-parent-cohorts", "Unknown P01 cohort proof")
    require(same(proof.get("context"), context), "Stale/mismatched P01 parent driver")
    scratch = Path(proof["scratch"])
    require(scratch == Path(context["log"]["path"]).parent / ("p01-cohort-" + context["verification_run_id"]),
            "P01 scratch binding")
    require(scratch.is_dir() and not scratch.is_symlink() and _mode(scratch) == 0o700,
            "P01 private scratch required")
    nonce = proof["driver_nonce"]
    require(isinstance(nonce, str) and re.fullmatch("[0-9a-f]{64}", nonce) is not None, "P01 fresh nonce")
    require((scratch / "COHORT").read_bytes() == nonce.encode()
            and not (scratch / "COHORT").is_symlink()
            and _mode(scratch / "COHORT") == 0o600, "P01 nonce marker")
    require(same(proof["scratch_binding"], scratch_binding(scratch)), "P01 fixture/nonce object replaced")
    sources = catalog(root)
    require(sources is not None and same(proof["source_before"], sources)
            and same(proof["source_after"], sources), "P01 source changed")
    require(same(proof["adapter"], binding(Path(__file__))), "P01 adapter changed")
    require(same(binding(context["tool"]["path"]), context["tool"]), "P01 tool changed")
    images, image, raw = build_images(root, context, target)
    p18 = proof["p18"]
    require(same(context.get("p18_environment"), p18["environment"] if p18 is not None else {}),
            "P18 Cargo process environment observation changed")
    require(same(p18, _p18_inherited(context["p18_environment"], raw)),
            "P18 inherited Cargo namespace/toolkit/proof changed")
    require(same(proof["images"], images), "P01 emitted image identity changed")
    owned_ids, discovered_total = discovery_observation(images, image, proof["discovery"], scratch, nonce, context)
    normal_ids, normal_outputs = normal_observation(images, proof["discovery"], proof["normal"], scratch, nonce, context, p18)
    require(same(proof["normal_observations"], normal_outputs), "Normal raw observation changed; reobservation required")
    commands = [r for pair in proof["discovery"] for r in pair.values()] + proof["normal"] + list(proof["parents"].values())
    require(all(r.get("cwd") == str(Path(root).resolve()) for r in commands), "P01 process cwd changed")
    skipped = []
    cmd = context["command"]
    for i, arg in enumerate(cmd):
        if arg == "--skip":
            require(i + 1 < len(cmd), "P01 skip argc")
            skipped.append(cmd[i + 1])
    require(skipped == list(EXCLUDED) and "--include-ignored" in cmd and "--test-threads=1" in cmd
            and "--message-format=json" in cmd, "Rust driver did not explicitly isolate P01 parents/helpers")
    normal = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", raw)
    require(normal and all(r[0] == "ok" and r[2:5] == ("0", "0", "0") for r in normal)
            and sum(int(r[5]) for r in normal) == 4 and sum(int(r[1]) for r in normal) > 0,
            "Wrong normal Rust/filtered outcomes")
    require(sum(int(r[1]) for r in normal) == discovered_total - 4, "Missing or extra normal Rust outcomes")
    cargo = libtest_observation(raw)
    require(all(status == "ok" for name, status in cargo["events"]), "Cargo named normal outcome did not pass")
    require(Counter(name for name, status in cargo["events"]) == Counter(row["method_id"] for row in normal_ids)
            and len(cargo["events"]) == discovered_total - 4, "Cargo raw normal identities differ from fresh image observations")
    require(not any(re.search(r"^test " + re.escape(n) + r" \.\.\.", raw, re.M) for n in EXCLUDED),
            "Private P01 entry or parent unexpectedly ran in normal selection")
    observations = []
    for kind in ("durable", "runtime"):
        observations.append(parent_observation(kind, proof["parents"][kind], image, scratch, nonce, len(owned_ids), context,
                                               PARENT_ONLY_DEADLINES[kind]))
    require(set(proof["parents"]) == {"durable", "runtime"}, "Unknown P01 parent")
    require(same(proof["observations"], observations), "P01 parent/case result changed")
    require(same(proof["source_after"], catalog(root)), "Late cohort source changed")
    require(same(proof["adapter"], binding(Path(__file__))), "Late cohort adapter changed")
    require(same(context["tool"], binding(context["tool"]["path"])), "Late cohort tool changed")
    require(same(p18, _p18_inherited(context["p18_environment"], raw)),
            "Late P18 Cargo namespace/toolkit/proof changed")
    _normal_final(proof)
    return {"parent_methods": 2, "helper_entries": 2, "child_passes": 0,
            "parent_subcases": 37, "normal_filtered_observations": 4,
            "normal_methods": len(normal_ids), "normal_identities": normal_ids,
            "normal_identity_sha256": hashlib.sha256(json.dumps(normal_ids, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            "normal_observations": normal_outputs,
            "proof_sha256": None, "scope": "bound parent controls; not direct child PASS or production/recovery acceptance"}


def qualify(root, record, expected_argv, run_id, source_sha256, target, proof_path, env, execute, tool):
    """Run only fixed parent/discovery commands, never receipt-provided commands."""
    root, proof_path = Path(root), Path(proof_path)
    context = prepare_record(record, expected_argv, run_id, source_sha256, tool)
    context["cwd"] = str(root.resolve())
    sources = catalog(root)
    require(sources is not None, "No P01 provider to qualify")
    images, image, raw = build_images(root, context, target)
    require(same(context.get("p18_environment"), {key: env[key] for key in P18_ENV if key in env}),
            "Missing/mismatched P18 Cargo process environment observation")
    p18 = _p18_inherited(env, raw)
    scratch = Path(context["log"]["path"]).parent / ("p01-cohort-" + run_id)
    scratch.mkdir(mode=0o700)  # exclusive; a stale directory is never reused
    nonce = secrets.token_hex(32)
    with (scratch / "COHORT").open("x", encoding="ascii") as f:
        f.write(nonce)
    (scratch / "COHORT").chmod(0o600)
    clean = dict(env)
    for name in OWN_ENV:
        clean.pop(name, None)
    for name in ("durable", "runtime", "tmp"):
        (scratch / name).mkdir(mode=0o700)
        if name != "tmp":
            (scratch / name / "tmp").mkdir(mode=0o700)
    clean.update(P01_RUN_NONCE=nonce, P01_DURABLE_SCRATCH=str(scratch / "durable"),
                 P01_RUNTIME_SCRATCH=str(scratch / "runtime"), TMPDIR=str(scratch / "tmp"))
    def invoke(name, argv, bound_image, timeout=10):
        invocation_env = dict(clean)
        namespace = None
        if p18 is not None and name.startswith("normal-"):
            namespace = _p18_namespace(scratch, name, bound_image, context, nonce, argv, p18, create=True)
            invocation_env.update(namespace["environment"])
        result = execute(name, argv, root, scratch / (name + ".log"), timeout, env=invocation_env)
        result["requested_timeout"] = timeout
        result["argc"] = len(argv)
        result["cwd"] = str(root.resolve())
        result["owned_environment"] = owned_environment(scratch, nonce)
        result["context_binding"] = {"phase": name, "verification_run_id": run_id,
            "source_sha256": source_sha256, "binary": bound_image["binary"], "driver_nonce": nonce}
        if namespace is not None:
            try:
                namespace["outputs"] = _p18_outputs(namespace["environment"], text(result["log"]["path"]),
                    parse_discovery(text(discovery[int(name.removeprefix("normal-"))]["all"]["log"]["path"])))
            except (OSError, ValueError) as error:
                result["p18_namespace_error"] = str(error)
            result["p18_namespace"] = namespace
        with (scratch / (name + ".json")).open("x", encoding="utf-8") as f:
            json.dump(result, f, sort_keys=True, indent=2)
        return result
    discovery = []
    for i, img in enumerate(images):
        binary = img["binary"]["path"]
        discovery.append({"all": invoke("discovery-all-" + str(i), [binary, "--list", "--format", "terse"], img),
                          "ignored": invoke("discovery-ignored-" + str(i), [binary, "--ignored", "--list", "--format", "terse"], img)})
    owned_ids, _ = discovery_observation(images, image, discovery, scratch, nonce, context)
    normals = [invoke("normal-" + str(i), normal_command(img, parse_discovery(text(discovery[i]["all"]["log"]["path"]))), img)
               for i, img in enumerate(images)]
    _, normal_outputs = normal_observation(images, discovery, normals, scratch, nonce, context, p18)
    parents = {}
    for kind in ("durable", "runtime"):
        parents[kind] = invoke("parent-" + kind, [image["binary"]["path"], PARENTS[kind], "--exact", "--nocapture", "--test-threads=1"],
                               image, PARENT_ONLY_DEADLINES[kind])
    observations = [parent_observation(k, parents[k], image, scratch, nonce, len(owned_ids), context, PARENT_ONLY_DEADLINES[k])
                    for k in ("durable", "runtime")]
    proof = {"schema": 3, "suite": "p01-parent-cohorts", "context": context, "p18": p18,
             "scratch": str(scratch), "driver_nonce": nonce, "source_before": sources,
             "source_after": catalog(root), "adapter": binding(Path(__file__)),
             "images": images, "discovery": discovery, "normal": normals, "normal_observations": normal_outputs,
             "parents": parents, "observations": observations,
             "scratch_binding": scratch_binding(scratch)}
    rebuild(root, context, proof, target)
    proof_path.parent.mkdir(parents=True, exist_ok=True)
    with proof_path.open("x", encoding="utf-8", newline="\n") as f:
        json.dump(proof, f, sort_keys=True, indent=2)
        f.write("\n")
    return context


def validate(root, proof_path, record, expected_argv, run_id, source_sha256, target, tool):
    proof_before = binding(proof_path)
    context = prepare_record(record, expected_argv, run_id, source_sha256, tool)
    context["cwd"] = str(Path(root).resolve())
    result = rebuild(root, context, read(proof_path), target)
    require(same(proof_before, binding(proof_path)), "Cohort proof changed during revalidation")
    result["proof_sha256"] = proof_before["sha256"]
    return result


def combine_summary(normal, qualification):
    require(type(normal.get("tests")) is int and normal["tests"] > 0
            and type(normal.get("passed")) is int and normal["passed"] == normal["tests"]
            and all(type(normal.get(k)) is int and normal[k] == 0 for k in ("skipped", "failures", "errors")),
            "Normal Rust summary is not qualified")
    require(same({k: qualification[k] for k in ("parent_methods", "helper_entries", "child_passes", "parent_subcases")},
                 {"parent_methods": 2, "helper_entries": 2, "child_passes": 0, "parent_subcases": 37}),
            "P01 coverage units mismatch")
    count = uint(qualification.get("normal_methods"), "Missing normal identity coverage")
    ids = qualification.get("normal_identities")
    require(type(ids) is list and len(ids) == count and count == normal["tests"]
            and len({json.dumps(row, sort_keys=True) for row in ids}) == count,
            "Normal summary differs from observed compound identities")
    for row in ids:
        require(type(row) is dict and set(row) == {"binary_path", "binary_sha256", "method_id", "source_sha256", "verification_run_id"}
                and all(type(value) is str for value in row.values()), "Invalid normal identity shape/type")
        require(Path(row["binary_path"]).is_absolute() and ".." not in Path(row["binary_path"]).parts
                and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9:]*", row["method_id"]) is not None
                and re.fullmatch("[0-9a-f]{64}", row["binary_sha256"]) is not None
                and re.fullmatch("[0-9a-f]{64}", row["source_sha256"]) is not None
                and re.fullmatch("[0-9a-f]{32}", row["verification_run_id"]) is not None,
                "Invalid normal identity binding")
    require(qualification.get("normal_identity_sha256") == hashlib.sha256(
        json.dumps(ids, sort_keys=True, separators=(",", ":")).encode()).hexdigest(), "Normal identity digest changed")
    return {**normal, "tests": normal["tests"] + 2, "passed": normal["passed"] + 2,
            "p01_parent_cohorts": qualification}


PARENT_ONLY_DEADLINES = {"durable": 330, "runtime": 60}  # <=10s/case plus bounded startup


def depfile_inputs(raw):
    """Rust dep-info prerequisites; preserve escaped spaces/backslashes in paths."""
    require("\x00" not in raw, "NUL in compiler depfile")
    result = set()
    for line in re.sub(r"\\\r?\n", "", raw).splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        require(":" in line, "Malformed compiler depfile")
        rhs = line.split(":", 1)[1]
        words, current, escaped = [], [], False
        for ch in rhs:
            if escaped:
                current.append(ch); escaped = False
            elif ch == "\\":
                escaped = True
            elif ch.isspace():
                if current:
                    words.append("".join(current)); current = []
            else:
                current.append(ch)
        require(not escaped, "Truncated compiler depfile escape")
        if current:
            words.append("".join(current))
        result.update(words)
    require(result, "Empty compiler depfile input set")
    return sorted(result)


def _record_file(row):
    actual = binding(row["path"])
    require(same({k: v for k, v in actual.items() if k != "kind"}, row),
            "Stale compiled file/tool/object")
    return actual


def _compiler_snapshot(root, build, spec):
    root, target, home = Path(root), Path(spec["target"]), Path(spec["cargo_home"])
    require(isinstance(spec["source"], dict) and spec["source"]
            and isinstance(spec["registry"], dict) and spec["registry"], "Missing compiler source/cache map")
    sources = {}
    for name, want in spec["source"].items():
        require(not Path(name).is_absolute() and ".." not in Path(name).parts, "Compiler source escape")
        value = binding(private_path(root / name, root))
        facts = {k: value[k] for k in ("sha256", "bytes", "mode")}
        require(same(facts, want), "Compiled project source changed")
        sources[name] = facts
    tools = {}
    for name in ("cargo", "rustc", "rustdoc"):
        row = build["tools"][name]
        require(Path(row["path"]) == Path(spec["tool_root"]) / name, "Wrong compiler tool path")
        value = _record_file(row)
        require(same({k: value[k] for k in ("sha256", "bytes", "mode")}, spec["tools"][name]),
                "Wrong compiler tool bytes")
        tools[name] = value
    require(isinstance(build["compiler_libraries"], dict) and build["compiler_libraries"],
            "Missing compiler library bindings")
    libraries = {}
    for name, row in build["compiler_libraries"].items():
        require(Path(name).is_relative_to(Path(spec["tool_root"]).parent / "lib"), "Wrong compiler library root")
        libraries[name] = _record_file(row)
    require(isinstance(build["depfiles"], dict) and build["depfiles"], "Missing compiler depfiles")
    deps, inputs = {}, {}
    for name, row in build["depfiles"].items():
        path = private_path(name, target)
        require(path.suffix == ".d", "Wrong compiler depfile kind")
        deps[name] = _record_file(row)
        for token in depfile_inputs(text(path)):
            path = Path(token)
            path = (path if path.is_absolute() else root / path).resolve(strict=True)
            if path.is_relative_to(root):
                name = path.relative_to(root).as_posix(); want = spec["source"].get(name)
                kind = "project"
            elif path.is_relative_to(home / "registry"):
                name = path.relative_to(home / "registry").as_posix(); want = spec["registry"].get(name)
                kind = "private-registry"
            else:
                require(path.is_relative_to(target), "Unbound external compiler input")
                want = None; kind = "generated"
            value = binding(path)
            if kind != "generated":
                require(want is not None and same({k: value[k] for k in ("sha256", "bytes", "mode")}, want),
                        "Compiler dependency differs from pinned copy")
            inputs[str(path)] = {"origin": kind, "file": value}
    require(all(str((root / name).resolve()) in inputs for name in SOURCE_PINS),
            "Parent/child source is absent from actual compiled input set")
    require(isinstance(build["emitted_artifacts"], list) and build["emitted_artifacts"],
            "Missing actual compiler artifacts")
    artifacts = []
    for row in build["emitted_artifacts"]:
        private_path(row["path"], target); artifacts.append(_record_file(row))
    return {"source": sources, "tools": tools, "libraries": libraries,
            "depfiles": deps, "compiled_inputs": inputs, "artifacts": artifacts}


def _parent_only_context(root, build, expected_argv, run_id, spec):
    require(isinstance(run_id, str) and re.fullmatch(r"[0-9a-f]{32}", run_id), "Parent-only run nonce")
    require(same(build.get("command"), expected_argv), "Parent-only build argv changed")
    required = ["test", "-p", "latexide-core", "--test", "recovery_contract", "--no-run",
                "--locked", "--offline", "--jobs", "1", "--message-format=json", "--verbose", "--verbose",
                "--target-dir", spec["target"]]
    require(expected_argv == [str(Path(spec["tool_root"]) / "cargo"), *required], "Wrong scoped build selection")
    require(build.get("name") == "p01-parent-only-build" and build.get("status") == "passed"
            and type(build.get("exit_code")) is int and build["exit_code"] == 0
            and build.get("timed_out") is False and build.get("target_was_absent") is True,
            "Parent-only build did not freshly pass")
    require(sys.platform == "darwin", "P01 actual parent fixture is macOS-only")
    require(build.get("cwd") == str(Path(root).resolve()) and build.get("target") == spec["target"],
            "Wrong parent-only source/target")
    require(same(build["source_before"], spec["source"]) and same(build["source_after"], spec["source"]),
            "Wrong parent-only source cohort")
    uint(build.get("duration_ms"), "Invalid parent-only build duration", 180000)
    require(build["log"].get("kind") == "command_log", "Wrong parent-only build log")
    log = _record_file({k: v for k, v in build["log"].items() if k != "kind"})
    return {"suite": "p01-build-only-cohorts", "build_name": build["name"], "command": expected_argv,
            "log": log, "run_id": run_id, "verification_run_id": run_id,
            "source_sha256": hashlib.sha256(json.dumps(spec["source"], sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            "source_root": str(Path(root).resolve()),
            "spec_sha256": hashlib.sha256(json.dumps(spec, sort_keys=True, separators=(",", ":")).encode()).hexdigest()}


def _fixture_inventory(scratch):
    files, dirs = {}, {}
    for path in sorted(scratch.rglob("*")):
        require(not path.is_symlink(), "Symlink in actual parent fixture")
        name = path.relative_to(scratch).as_posix()
        if path.is_dir():
            s = path.lstat()
            dirs[name] = {"device": s.st_dev, "inode": s.st_ino, "uid": s.st_uid, "mode": _mode(path)}
        else:
            files[name] = binding(path)
    require(files, "Empty actual parent fixture")
    return {"files": files, "directories": dirs}



def _normal_negative_link_role(scratch, relative, pid, raw_target):
    """Compare inert native path components; never resolve or follow a link."""
    uint(pid, "Actual normal fixture PID type", 2**31 - 1)
    require(pid > 0 and isinstance(raw_target, str) and "\0" not in raw_target,
            "Invalid normal negative-link identity")
    target = raw_target
    if isinstance(scratch, PureWindowsPath) and target.startswith("\\\\?\\"):
        target = target[4:]
        if target.upper().startswith("UNC\\"):
            target = "\\\\" + target[4:]
        else:
            require(re.match(r"[A-Za-z]:[\\/]", target), "Unsupported Windows link target prefix")
    target = type(scratch)(target)
    require(target.is_absolute() and ".." not in target.parts, "Nonabsolute/traversing normal link target")
    path = scratch / relative
    role, wanted = None, None
    if re.fullmatch(r"durable/tmp/path-fixture-" + str(pid) + r"-\d+/alias", relative):
        role, wanted = "durable-alias", path.parent
    elif re.fullmatch(r"durable/tmp/ancestor-initial-directory-" + str(pid) + r"-\d+/parent", relative):
        role, wanted = "durable-ancestor", path.parent / "outside"
    elif re.fullmatch(r"durable/tmp/(checkpoint|journal)-" + str(pid) + r"-\d+/store/(checkpoint|journal)-1.rec", relative):
        kind = relative.split('/')[2].split('-')[0]
        require(path.name == kind + "-1.rec", "Mismatched normal corrupt-store role")
        parent = scratch / "durable/tmp"
        require(target.is_relative_to(parent), "Normal corrupt-store target escaped its fixture owner")
        parts = target.relative_to(parent).parts
        require(len(parts) == 2 and re.fullmatch(r"path-fixture-" + str(pid) + r"-\d+", parts[0])
                and parts[1] == "own-sentinel", "Wrong normal corrupt-store link target")
        role, wanted = "durable-" + kind, target
    elif re.fullmatch(r"runtime/tmp/runtime-" + str(pid) + r"-\d+/project/main.tex", relative):
        role, wanted = "runtime-project", path.parent.parent / "own-outside-sentinel"
    require(role is not None and target == wanted, "Unknown/parent or misbound normal fixture symlink")
    return role


def _actual_fixture_inventory(scratch, normals):
    """Custody of five source-defined normal negative-fixture links, no following.

    Parent fixtures and unknown links remain invalid. These inert link nodes
    are raw test data, never executable paths or successful helper evidence.
    SOURCE_PINS/catalog and the normal phase are checked by the caller first.
    """
    scratch = Path(scratch)
    require(type(normals) is list and len(normals) == 1 and normals[0].get("name") == "normal-0",
            "Actual normal fixture owner missing")
    record = normals[0]
    pid = uint(record.get("own_pid"), "Actual normal fixture PID type", 2**31 - 1)
    require(pid > 0 and same(record, read(private_path(scratch / "normal-0.json", scratch))),
            "Actual normal fixture PID/record changed")
    files, dirs, links, roles = {}, {}, {}, Counter()
    for base, directories, names in os.walk(scratch, followlinks=False):
        for name in sorted(directories + names):
            path = Path(base) / name
            relative = path.relative_to(scratch).as_posix()
            s = path.lstat()
            if stat.S_ISLNK(s.st_mode):
                if name in directories:
                    directories.remove(name)
                target = os.readlink(path)
                role = _normal_negative_link_role(scratch, relative, pid, target)
                raw = os.fsencode(target)
                links[relative] = {"path": str(path), "kind": "symlink", "target": target,
                    "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw),
                    "device": s.st_dev, "inode": s.st_ino, "uid": s.st_uid,
                    "mode": stat.S_IMODE(s.st_mode), "role": role, "normal_own_pid": pid}
                roles[role] += 1
            elif stat.S_ISDIR(s.st_mode):
                dirs[relative] = {"device": s.st_dev, "inode": s.st_ino, "uid": s.st_uid, "mode": _mode(path)}
            else:
                files[relative] = binding(path)
    require(roles == Counter({"durable-alias": 1, "durable-ancestor": 1, "durable-checkpoint": 1,
                              "durable-journal": 1, "runtime-project": 1}),
            "Missing/duplicate source-defined normal negative-fixture link")
    require(files, "Empty actual fixture inventory")
    return {"files": files, "directories": dirs, "normal_negative_symlinks": links}


def _parent_only_final(proof, scratch):
    for role in ("compiler", "fixture"):
        require(same(proof[role], binding(proof[role]["path"])), "Late actual " + role + " receipt changed")
    inventory = (_actual_fixture_inventory(scratch, proof["normal"])
                 if proof.get("suite") == "p01-build-only-cohorts" else _fixture_inventory(scratch))
    require(same(read(proof["fixture"]["path"]), inventory),
            "Late nested actual parent fixture change")


def _execution_binding(spec, run_id, expected_argv):
    path = Path(spec["execution_receipt"]); execution = read(path)
    require(execution.get("run_id") == run_id and execution.get("source_copied_equal") is True
            and same(execution.get("expected_build_argv"), expected_argv)
            and same(execution.get("parent_IDs"), PARENTS) and same(execution.get("helper_IDs"), CHILDREN)
            and same(execution.get("parent_deadlines"), PARENT_ONLY_DEADLINES)
            and same(execution.get("normal_policy"), {"required": True, "timeout": 10, "excluded": list(EXCLUDED)}),
            "Wrong retained execution provenance")
    source = {}
    for name, want in execution["adapter_source"].items():
        require(not Path(name).is_absolute() and ".." not in Path(name).parts, "Execution source escape")
        value = binding(Path(execution["adapter_root"]) / name)
        require(same({k: value[k] for k in ("sha256", "bytes", "mode")}, want),
                "Retained execution adapter source changed")
        source[name] = value
    require(source, "Missing retained execution adapter source")
    driver = binding(execution["argv"][2])
    require(same({k: driver[k] for k in ("sha256", "bytes", "mode")}, execution["script"]),
            "Retained execution driver changed")
    return {"receipt": binding(path), "adapter_source": source, "driver": driver}


def _actual_bundle_path(proof_path, role, run_id):
    """Each validator seal owns new receipts while retained raw data stays fixed."""
    proof_path = Path(proof_path)
    require(role in ("compiler", "fixture") and re.fullmatch(r"[0-9a-f]{32}", run_id)
            and re.fullmatch(r"[A-Za-z0-9_-]{1,96}", proof_path.stem), "Invalid actual bundle identity")
    return proof_path.parent / (role + "-" + run_id + "-" + proof_path.stem + ".json")


def parent_only_validate(root, build, expected_argv, run_id, spec, proof_path):
    """Read/hash/parse a build-only image's fresh normal and parent cohorts.

    This distinct private schema has no Cargo normal-summary claim. Old parent-
    only proofs are historical inputs and are never upgraded by validation.
    """
    proof_path = Path(proof_path); proof_before = binding(proof_path); proof = read(proof_path)
    require(isinstance(proof, dict) and set(proof) == {
        "schema", "suite", "context", "adapter", "sources", "scratch", "nonce", "scratch_binding",
        "compiler", "fixture", "images", "discovery", "normal", "normal_observations",
        "parents", "observations", "execution"}, "Unknown build-only cohort proof fields")
    require(type(proof["schema"]) is int and proof["schema"] == 2
            and proof["suite"] == "p01-build-only-cohorts", "Wrong build-only cohort proof scope")
    context = _parent_only_context(root, build, expected_argv, run_id, spec)
    require(same(proof["context"], context) and same(proof["adapter"], binding(Path(__file__))),
            "Build-only context/adapter changed")
    require(same(proof["execution"], _execution_binding(spec, run_id, expected_argv)),
            "Retained execution provenance changed")
    sources = catalog(root); require(sources is not None and same(proof["sources"], sources), "P01 source contract changed")
    scratch = proof_path.parent / ("p01-parent-only-" + run_id)
    require(proof["scratch"] == str(scratch) and _mode(scratch) == 0o700, "Wrong actual scratch root")
    nonce = proof["nonce"]
    require(isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{64}", nonce), "Wrong actual nonce")
    require((scratch / "COHORT").read_bytes() == nonce.encode()
            and _mode(scratch / "COHORT") == 0o600
            and same(proof["scratch_binding"], scratch_binding(scratch)), "Actual root/nonce object changed")
    for role in ("compiler", "fixture"):
        expected = _actual_bundle_path(proof_path, role, run_id)
        require(proof[role]["path"] == str(expected.resolve()) and same(proof[role], binding(expected)),
                "Stale actual " + role + " receipt")
    compiler = read(proof["compiler"]["path"])
    require(same(compiler, _compiler_snapshot(root, build, spec)), "Actual compiled input changed")
    images, image, _ = build_images(root, context, spec["target"])
    require(len(images) == 1 and same(proof["images"], images), "Actual image changed")
    ids, total = discovery_observation(images, image, proof["discovery"], scratch, nonce, context)
    normal_ids, normal_outputs = normal_observation(images, proof["discovery"], proof["normal"], scratch, nonce, context)
    require(same(proof["normal_observations"], normal_outputs), "Normal raw observation changed; reobservation required")
    records = [r for pair in proof["discovery"] for r in pair.values()] + proof["normal"] + list(proof["parents"].values())
    require(all(r.get("cwd") == str(Path(root).resolve()) for r in records), "Wrong actual process cwd")
    require(set(proof["parents"]) == {"durable", "runtime"}, "Missing actual parent")
    observations = [parent_observation(k, proof["parents"][k], image, scratch, nonce, len(ids), context, PARENT_ONLY_DEADLINES[k])
                    for k in ("durable", "runtime")]
    require(same(proof["observations"], observations), "Actual parent observation changed")
    require(same(read(proof["fixture"]["path"]), _actual_fixture_inventory(scratch, proof["normal"])), "Actual nested fixture changed")
    require(same(compiler, _compiler_snapshot(root, build, spec)), "Late compiled input change")
    require(same(proof["execution"], _execution_binding(spec, run_id, expected_argv)),
            "Late retained execution provenance change")
    require(same(proof["adapter"], binding(Path(__file__))), "Late actual validator source change")
    _normal_final(proof)
    # P01_PARENT_ONLY_FINAL_GUARD: after the final compiler/source/normal read.
    _parent_only_final(proof, scratch)
    require(same(proof_before, binding(proof_path)), "Actual proof changed during revalidation")
    return {"suite": "p01-build-only-cohorts", "parent_methods": 2, "helper_entries": 2,
            "parent_subcases": 37, "child_passes": 0, "ordinary_Rust_methods": len(normal_ids),
            "normal_methods": len(normal_ids), "normal_identities": normal_ids,
            "normal_identity_sha256": hashlib.sha256(json.dumps(normal_ids, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            "normal_observations": normal_outputs, "normal_filtered_observations": total - len(normal_ids),
            "discovered_test_IDs": total, "exact_selection_filtered_observations": 2 * (len(ids) - 1),
            "proof_sha256": proof_before["sha256"], "Cargo_normal_summary_observed": False,
            "scope": "actual targeted normal and parent cohorts; not full-workspace, platform or product acceptance"}

def _write_parent_bundle(path, value):
    raw = (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()
    require(len(raw) <= 1024 * 1024, "Actual evidence bundle quota")
    with path.open("xb") as f:
        f.write(raw)


def parent_only_qualify(root, build, expected_argv, run_id, spec, proof_path, env, execute):
    """Execute fixed discovery, normal selection and two parents from one fresh image."""
    root, proof_path = Path(root), Path(proof_path)
    context = _parent_only_context(root, build, expected_argv, run_id, spec)
    _execution_binding(spec, run_id, expected_argv)
    compiler = _compiler_snapshot(root, build, spec)
    images, image, _ = build_images(root, context, spec["target"])
    require(len(images) == 1, "Scoped parent-only build emitted extra test images")
    scratch = proof_path.parent / ("p01-parent-only-" + run_id)
    scratch.mkdir(mode=0o700)
    nonce = secrets.token_hex(32)
    with (scratch / "COHORT").open("x") as f:
        f.write(nonce)
    (scratch / "COHORT").chmod(0o600)
    for name in ("durable", "runtime", "tmp"):
        (scratch / name).mkdir(mode=0o700)
        if name != "tmp":
            (scratch / name / "tmp").mkdir(mode=0o700)
    clean = dict(env)
    for name in OWN_ENV:
        clean.pop(name, None)
    clean.update(owned_environment(scratch, nonce))
    def invoke(name, argv, timeout=10):
        value = execute(name, argv, root, scratch / (name + ".log"), timeout, env=clean)
        value.update(argc=len(argv), cwd=str(root.resolve()), requested_timeout=timeout,
                     owned_environment=owned_environment(scratch, nonce),
                     context_binding={"phase": name, "verification_run_id": run_id,
                         "source_sha256": context["source_sha256"], "binary": image["binary"], "driver_nonce": nonce})
        with (scratch / (name + ".json")).open("x") as f:
            json.dump(value, f, sort_keys=True, indent=2)
        return value
    discovery = [{"all": invoke("discovery-all-0", [image["binary"]["path"], "--list", "--format", "terse"]),
                  "ignored": invoke("discovery-ignored-0", [image["binary"]["path"], "--ignored", "--list", "--format", "terse"])}]
    ids, _ = discovery_observation(images, image, discovery, scratch, nonce, context)
    normals = [invoke("normal-0", normal_command(image, ids))]
    _, normal_outputs = normal_observation(images, discovery, normals, scratch, nonce, context)
    parents = {k: invoke("parent-" + k, [image["binary"]["path"], PARENTS[k], "--exact", "--nocapture", "--test-threads=1"],
                         PARENT_ONLY_DEADLINES[k]) for k in ("durable", "runtime")}
    observations = [parent_observation(k, parents[k], image, scratch, nonce, len(ids), context, PARENT_ONLY_DEADLINES[k])
                    for k in ("durable", "runtime")]
    compiler_path = _actual_bundle_path(proof_path, "compiler", run_id)
    fixture_path = _actual_bundle_path(proof_path, "fixture", run_id)
    _write_parent_bundle(compiler_path, compiler)
    _write_parent_bundle(fixture_path, _actual_fixture_inventory(scratch, normals))
    proof = {"schema": 2, "suite": "p01-build-only-cohorts", "context": context, "adapter": binding(Path(__file__)),
             "sources": catalog(root), "scratch": str(scratch), "nonce": nonce,
             "scratch_binding": scratch_binding(scratch), "compiler": binding(compiler_path),
             "fixture": binding(fixture_path), "images": images, "discovery": discovery,
             "normal": normals, "normal_observations": normal_outputs,
             "parents": parents, "observations": observations,
             "execution": _execution_binding(spec, run_id, expected_argv)}
    _write_parent_bundle(proof_path, proof)
    return parent_only_validate(root, build, expected_argv, run_id, spec, proof_path)


def parent_only_seal(root, build, expected_argv, run_id, spec, proof_path):
    """Seal retained own raw records without running/replaying a process.

    The execution adapter and this validator may be different versions; both
    are retained and bound. Header-only child output is never a raw-ID claim.
    """
    root, proof_path = Path(root), Path(proof_path)
    context = _parent_only_context(root, build, expected_argv, run_id, spec)
    execution = _execution_binding(spec, run_id, expected_argv)
    compiler = _compiler_snapshot(root, build, spec)
    images, image, _ = build_images(root, context, spec["target"])
    require(len(images) == 1, "Scoped parent-only build emitted extra test images")
    scratch = proof_path.parent / ("p01-parent-only-" + run_id)
    require(not scratch.is_symlink() and _mode(scratch) == 0o700, "Missing retained own scratch")
    nonce = (scratch / "COHORT").read_text()
    require(re.fullmatch(r"[0-9a-f]{64}", nonce), "Missing retained nonce")
    discovery = [{"all": read(scratch / "discovery-all-0.json"),
                  "ignored": read(scratch / "discovery-ignored-0.json")}]
    ids, _ = discovery_observation(images, image, discovery, scratch, nonce, context)
    normals = [read(scratch / "normal-0.json")]
    _, normal_outputs = normal_observation(images, discovery, normals, scratch, nonce, context)
    parents = {k: read(scratch / ("parent-" + k + ".json")) for k in ("durable", "runtime")}
    observations = [parent_observation(k, parents[k], image, scratch, nonce, len(ids), context, PARENT_ONLY_DEADLINES[k])
                    for k in ("durable", "runtime")]
    compiler_path = _actual_bundle_path(proof_path, "compiler", run_id)
    fixture_path = _actual_bundle_path(proof_path, "fixture", run_id)
    _write_parent_bundle(compiler_path, compiler)
    _write_parent_bundle(fixture_path, _actual_fixture_inventory(scratch, normals))
    proof = {"schema": 2, "suite": "p01-build-only-cohorts", "context": context, "adapter": binding(Path(__file__)),
             "sources": catalog(root), "scratch": str(scratch), "nonce": nonce,
             "scratch_binding": scratch_binding(scratch), "compiler": binding(compiler_path),
             "fixture": binding(fixture_path), "images": images, "discovery": discovery,
             "normal": normals, "normal_observations": normal_outputs,
             "parents": parents, "observations": observations, "execution": execution}
    _write_parent_bundle(proof_path, proof)
    return parent_only_validate(root, build, expected_argv, run_id, spec, proof_path)


def main():
    p = argparse.ArgumentParser(description="Read-only P01 cohort source inspection; no provider execution")
    p.add_argument("--inspect", type=Path, required=True)
    a = p.parse_args()
    try:
        found = catalog(a.inspect)
        print(json.dumps({"schema": 1, "applicable": found is not None,
                          "source_files": found or {}, "execution_performed": False,
                          "scope": "source inspection only, not cohort acceptance"}, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as e:
        print(json.dumps({"status": "failed", "reason": str(e)}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
