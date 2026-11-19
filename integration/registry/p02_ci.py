"""Caller-owned P02 test fixtures and read-only evidence validation.

This binds local observations, not process authentication or an OS sandbox.
Only verify/P01's fixed commands execute; stored evidence is never executed.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
from collections import Counter

ENV = ("P02_RUNTIME_FIXTURES", "P02_WORKSPACE_FIXTURES", "P02_RUN_NONCE")
SOURCE_PINS = {'crates/latexide-core/src/lib.rs': {'sha256': '50c143c8189d4f0252f19cd3008e734e95f543e4bc5f8fa4de496aa1e38dbe27', 'bytes': 1519, 'mode': 420}, 'crates/latexide-core/src/services/recovery/runtime_binding.rs': {'sha256': '7df61324995d08e70c398bbb733033668c2fef3310f8f21660f26fa08b1aa01f', 'bytes': 35641, 'mode': 420}, 'crates/latexide-core/src/services/transactions/mod.rs': {'sha256': '77d0da0bf47f261d4c7e23eafb90c0a6ebf16d0c7790c79623177ecdedfe69e6', 'bytes': 8411, 'mode': 420}, 'crates/latexide-core/src/services/transactions/sha256.rs': {'sha256': '21392948e08b68e4bbb3d08cc1b940c46f1e90ce3794f05eec1bd6b20dd2f2c0', 'bytes': 3200, 'mode': 420}, 'crates/latexide-core/src/services/transactions/workspace_bridge.rs': {'sha256': '5afb2bc0d3ef00a9bed362f7418116752880b4b2647db8b87fbc76d12e9d8176', 'bytes': 3596, 'mode': 420}, 'crates/latexide-core/src/services/vfs/filesystem.rs': {'sha256': '882dfa223342bdb2483e9f92d864e40fe40e62e96f1f9320c7a8c1f0e2cbf394', 'bytes': 20059, 'mode': 420}, 'crates/latexide-core/src/services/vfs/invalidation.rs': {'sha256': '00071fedf07f07d74ae248049c684f68648f87ba6545128159ad690315209b1c', 'bytes': 3863, 'mode': 420}, 'crates/latexide-core/src/services/vfs/mod.rs': {'sha256': '1e5bdb67c5e073a2f9f604d57b0baa2d5a4fc2e092544024f31389613d130b2d', 'bytes': 15375, 'mode': 420}, 'crates/latexide-core/src/workspace.rs': {'sha256': 'ea60b3ee9fae416967d111d46fe49e17a6b33c4b3bfb24172968f2034df6553e', 'bytes': 32003, 'mode': 420}, 'crates/latexide-core/src/workspace_analysis_capture.rs': {'sha256': 'd9a8e89230e459d1d656a2d20bc86ddb28405ce7237cb98ab80c3542621b7d29', 'bytes': 8928, 'mode': 420}, 'crates/latexide-core/src/workspace_capture.rs': {'sha256': 'e646de9c0ed9edc44fb1c822c868a9c7e4e92ad35e14979a7f3d6795d5cb0632', 'bytes': 6713, 'mode': 420}, 'crates/latexide-core/tests/transaction_contract.rs': {'sha256': '2ce636a1c2aa958207b1299652f67eea4901a8315e142f48d20faf241539e4a1', 'bytes': 30743, 'mode': 420}, 'crates/latexide-core/tests/transaction_workspace_contract.rs': {'sha256': 'b47680b137e9a4e986a0ab682ae9ac404a83d36d2f69a1a2bf50d086388fa210', 'bytes': 14883, 'mode': 420}, 'crates/latexide-ffi/src/runtime.rs': {'sha256': 'd51c0e268a9cd04074150aa76faefa6003eb83b4b26cc223f6563b9b2e87dae2', 'bytes': 16173, 'mode': 420}, 'crates/latexide-ffi/src/runtime_transactions.rs': {'sha256': '3e0ac395a8ec762d642d6f439f1ed5a1674a5f35a62ce2394af9217ccd9fed7d', 'bytes': 10228, 'mode': 420}, 'crates/latexide-ffi/src/runtime_transactions_tests.rs': {'sha256': '68f46d6a27e8603d80c5cade5c5200b1f0bb251589cc519eb3e79f25beea46fc', 'bytes': 37207, 'mode': 420}}
NEW_PATHS = (
    "crates/latexide-core/src/services/transactions/mod.rs",
    "crates/latexide-core/src/services/transactions/sha256.rs",
    "crates/latexide-core/src/services/transactions/workspace_bridge.rs",
    "crates/latexide-core/src/services/vfs/filesystem.rs",
    "crates/latexide-core/src/services/vfs/invalidation.rs",
    "crates/latexide-core/src/workspace_analysis_capture.rs",
    "crates/latexide-core/src/workspace_capture.rs",
    "crates/latexide-core/tests/transaction_contract.rs",
    "crates/latexide-core/tests/transaction_workspace_contract.rs",
    "crates/latexide-ffi/src/runtime_transactions.rs",
    "crates/latexide-ffi/src/runtime_transactions_tests.rs",
)
CANONICAL = "crates/latexide-core/tests/transaction_contract.rs"
WORKSPACE = "crates/latexide-core/tests/transaction_workspace_contract.rs"
RUNTIME = "crates/latexide-ffi/src/runtime_transactions_tests.rs"
FFI = "runtime::transactions::tests::"
ROLES = {"canonical": CANONICAL, "workspace": WORKSPACE,
         "ffi": "crates/latexide-ffi/src/lib.rs"}
_parser = None


def require(ok, message):
    if not ok:
        raise ValueError("P02 " + message)


def same(a, b):
    return json.dumps(a, sort_keys=True, separators=(",", ":")) == json.dumps(b, sort_keys=True, separators=(",", ":"))


def _mode(path):
    return stat.S_IMODE(Path(path).stat().st_mode)


def _stable_file_stats(path, before, opened, after, windows):
    fields = ("st_dev", "st_ino", "st_size", "st_mode", "st_uid", "st_nlink", "st_mtime_ns")
    require(all(getattr(before, key) == getattr(after, key) for key in fields),
            "file path changed while reading")
    require(all(getattr(before, key) == getattr(opened, key) for key in fields if key != "st_mode"),
            "file descriptor identity changed while reading")
    descriptor_mode = before.st_mode
    if windows and path.suffix.lower() in (".exe", ".bat", ".cmd", ".com"):
        # CPython adds filename-derived execute bits to Windows path stat;
        # fstat's handle view has no filename. Keep path-before/after modes
        # exact and compare every real object/size/link/time field above.
        # https://github.com/python/cpython/blob/v3.13.17/Modules/posixmodule.c#L1843
        descriptor_mode &= ~0o111
    require(opened.st_mode == descriptor_mode, "file descriptor mode changed while reading")


def binding(path):
    path = Path(path); before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and not path.is_symlink(), "regular file required")
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        opened = os.fstat(stream.fileno())
    after = path.lstat()
    _stable_file_stats(path, before, opened, after, os.name == "nt")
    return {"path": str(path.resolve()), "sha256": digest, "bytes": after.st_size,
            "mode": _mode(path), "device": after.st_dev, "inode": after.st_ino,
            "uid": after.st_uid, "kind": "file"}


def directory_binding(path, private=False):
    path = Path(path); value = path.lstat()
    require(stat.S_ISDIR(value.st_mode) and not path.is_symlink(), "directory required")
    if private:
        # Windows chmod does not implement POSIX privacy. Its actual identity is
        # still bound; this adapter makes no Windows ACL/containment claim.
        require(_mode(path) == (0o777 if os.name == "nt" else 0o700), "private directory mode")
    return {"path": str(path.resolve()), "device": value.st_dev, "inode": value.st_ino,
            "uid": value.st_uid, "mode": _mode(path)}


def owned(path, owner):
    path, owner = Path(path), Path(owner)
    require(path.is_absolute() and owner.is_absolute() and ".." not in path.parts
            and ".." not in owner.parts and path.is_relative_to(owner), "lexical owner required")
    require(not owner.is_symlink() and path.resolve(strict=True).is_relative_to(owner.resolve(strict=True)), "foreign owner")
    current = owner
    for part in path.relative_to(owner).parts:
        current /= part
        require(not current.is_symlink(), "symlink in owned path")
    return path


def read(path, limit=4 * 1024 * 1024):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate evidence field")
            result[key] = value
        return result
    with Path(path).open("rb") as stream:
        raw = stream.read(limit + 1)
    require(len(raw) <= limit, "evidence quota")
    return json.loads(raw, object_pairs_hook=unique)


def text(path):
    with Path(path).open("rb") as stream:
        raw = stream.read(16 * 1024 * 1024 + 1)
    require(len(raw) <= 16 * 1024 * 1024, "log quota")
    return raw.decode("utf-8", errors="strict")


def write(path, value):
    raw = (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
    require(len(raw) <= 4 * 1024 * 1024, "output quota")
    with Path(path).open("xb") as stream:
        stream.write(raw)
    Path(path).chmod(0o600)


def catalog(root):
    root = Path(root)
    if not any((root / name).exists() or (root / name).is_symlink() for name in NEW_PATHS):
        return None
    require(len(SOURCE_PINS) == 16 and set(NEW_PATHS).issubset(SOURCE_PINS), "partial source catalog")
    rows = {}
    for name, expected in SOURCE_PINS.items():
        row = binding(owned(root / name, root))
        expected = dict(expected)
        if os.name == "nt":
            require(expected["mode"] == 0o644, "nonportable source mode contract")
            expected["mode"] = 0o666
        require(same({k: row[k] for k in ("sha256", "bytes", "mode")}, expected), "source pin changed: " + name)
        rows[name] = row
    return rows


def platform_contract():
    mac = sys.platform == "darwin"
    return {"target": sys.platform, "runtime_methods": 27, "shared_memory_redo": True,
        "persistent_recovery_positive": mac, "unconfirmed_recovery_refusal": not mac}


def _test_cases(root, name):
    source = text(Path(root) / name)
    pattern = r'(?m)^(?:#\[cfg\(([^\n]+)\)\]\n)?#\[test\]\nfn ([A-Za-z_][A-Za-z_0-9]*)\s*\('
    found = list(re.finditer(pattern, source)); rows = []
    for index, match in enumerate(found):
        cfg, method = match.groups()
        previous = source[:match.start()].splitlines()
        require(cfg is not None or not (previous and previous[-1].startswith("#[cfg")), "unknown source test condition")
        require(cfg in (None, 'target_os = "macos"', 'not(target_os = "macos")'), "unregistered source condition")
        active = cfg is None or (cfg == 'target_os = "macos"') == (sys.platform == "darwin")
        rows.append({"method": method, "cfg": cfg, "active": active,
            "body": source[match.end():found[index + 1].start() if index + 1 < len(found) else len(source)]})
    return rows


def methods(root):
    require(catalog(root) is not None, "missing provider")
    result = {}
    for role, name, total, active in (("canonical", CANONICAL, 32, 32), ("workspace", WORKSPACE, 8, 8), ("runtime", RUNTIME, 28, 27)):
        rows = _test_cases(root, name); names = [row["method"] for row in rows]
        require(len(names) == len(set(names)) == total, "missing/duplicate source methods")
        conditions = {row["method"]: row["cfg"] for row in rows if row["cfg"] is not None}
        require(conditions == ({"m03_common_redo_group": 'target_os = "macos"',
            "m03_recovery_refuses_unconfirmed_platform": 'not(target_os = "macos")'} if role == "runtime" else {}), "changed platform/source test contract")
        result[role] = sorted(row["method"] for row in rows if row["active"])
        require(len(result[role]) == active, "zero/partial platform methods")
    result["ffi"] = sorted([*(FFI + n for n in result["runtime"]),
        *(FFI + "canonical_regression::" + n for n in result["canonical"]),
        *(FFI + "workspace_regression::" + n for n in result["workspace"])])
    return result


def role(root, image):
    src = Path(image["src_path"])
    matches = [key for key, name in ROLES.items() if src.resolve() == (Path(root) / name).resolve()]
    require(len(matches) == 1, "unknown image source")
    require(same(image["binary"], binding(image["binary"]["path"])), "image changed")
    return matches[0]


def selected(root, image, ids):
    if catalog(root) is None:
        return False
    # Unqualified libtest names can occur in unrelated integration images.
    # Select the source-bound image first, then require its complete method set.
    if not any(Path(image["src_path"]).resolve() == (Path(root) / name).resolve() for name in ROLES.values()):
        return False
    key = role(root, image)
    actual = [name for name in ids if name.startswith(FFI)] if key == "ffi" else ids
    require(len(actual) == len(set(actual)) and set(actual) == set(methods(root)[key]),
            "partial/foreign P02 image selection")
    return True


def normal_parent(scratch, nonce, context, discovery):
    require(re.fullmatch(r"[0-9a-f]{64}", nonce) is not None, "parent nonce")
    return {"kind": "p01-normal", "scratch": directory_binding(scratch, True), "driver_nonce": nonce,
        "cargo_log": context["log"], "cargo_command": context["command"],
        "p18_environment": context.get("p18_environment", {}),
        "discovery": {key: binding(discovery[key]["log"]["path"]) for key in ("all", "ignored")}}


def _spec(root, directory, phase, context, command, parent, binary):
    require(re.fullmatch(r"[0-9a-f]{32}", context["verification_run_id"]) is not None, "run identity")
    require(re.fullmatch(r"[0-9a-f]{64}", context["source_sha256"]) is not None, "source identity")
    require(type(command) is list and command and all(type(v) is str and v for v in command), "fixed command")
    require(phase == "cargo" or re.fullmatch(r"(?:normal|selected)-\d+", phase), "phase identity")
    require(type(parent) is dict and parent and parent.get("kind") in ("workspace-cargo", "p01-normal", "local-build-only"), "parent identity")
    require(same(context["tool"], binding(context["tool"]["path"])), "tool changed")
    if binary is not None:
        require(same(binary, binding(binary["path"])) and command[0] == binary["path"], "command/image binding")
    require((phase == "cargo") == (binary is None), "phase/image binding")
    value = {"phase": phase, "directory": str(directory), "owner": directory_binding(directory.parent),
        "verification_run_id": context["verification_run_id"], "source_sha256": context["source_sha256"],
        "tool": context["tool"], "command": command, "binary": binary, "parent": parent,
        "sources": catalog(root), "adapter": binding(Path(__file__)), "platform_contract": platform_contract()}
    require(value["sources"] is not None, "unbound namespace")
    value["nonce"] = hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return value


def create(root, directory, phase, context, command, parent, binary=None):
    require(catalog(root) is not None, "unregistered fixture creation")
    directory = Path(directory).absolute()
    require(directory == directory.resolve() and ".." not in directory.parts, "canonical namespace path")
    spec = _spec(root, directory, phase, context, command, parent, binary)
    directory.mkdir(mode=0o700)  # exclusive; stale input is never reused
    for name in ("runtime", "workspace"):
        (directory / name).mkdir(mode=0o700)
    write(directory / "NAMESPACE.json", spec)
    return {"spec": spec, "metadata": binding(directory / "NAMESPACE.json"),
        "directories": [directory_binding(directory / name, True) for name in (".", "runtime", "workspace")],
        "environment": dict(zip(ENV, (str(directory / "runtime"), str(directory / "workspace"), spec["nonce"])))}


def _parser_module():
    global _parser
    if _parser is None:
        spec = importlib.util.spec_from_file_location("latexide_p02_libtest", Path(__file__).with_name("p01_ci.py"))
        _parser = importlib.util.module_from_spec(spec); spec.loader.exec_module(_parser)
    return _parser


def _libtest(raw):
    return _parser_module().libtest_observation(raw)


def discovery_ids(record, binary, ignored=False):
    require(type(record) is dict and type(record.get("log")) is dict, "discovery record/log shape")
    command = [binary["path"], *(["--ignored"] if ignored else []), "--list", "--format", "terse"]
    require(same(record.get("command"), command) and record.get("status") == "passed"
            and type(record.get("exit_code")) is int and record["exit_code"] == 0 and record.get("timed_out") is False,
            "discovery command/result")
    require(record.get("log", {}).get("sha256") == binding(record["log"]["path"])["sha256"], "stale discovery log")
    raw = text(record["log"]["path"])
    # Use the unchanged strict discovery parser, including duplicate/unknown guards.
    return _parser_module().parse_discovery(raw)


def _fixture_labels(root, name):
    source = "\n".join(row["body"] for row in _test_cases(root, name) if row["active"])
    labels = re.findall(r'(?:project|multi_setup)\("([A-Za-z_0-9-]+)"', source)
    require(labels and len(labels) == len(set(labels)), "source fixture label identity")
    return set(labels)


def image_observation(root, image, discovery, raw, environment, expected_pid=None):
    key = role(root, image); wanted = methods(root)[key]
    require(type(discovery) is dict and set(discovery) == {"all", "ignored"}, "discovery roles")
    ids = discovery_ids(discovery["all"], image["binary"])
    ignored = discovery_ids(discovery["ignored"], image["binary"], True)
    actual = [n for n in ids if n.startswith(FFI)] if key == "ffi" else ids
    require(len(actual) == len(set(actual)) and set(actual) == set(wanted) and not (set(wanted) & set(ignored)),
            "zero/missing/unknown/ignored discovered methods")
    result = _libtest(raw)
    events = [(n, state) for n, state in result["events"] if n.startswith(FFI)] if key == "ffi" else result["events"]
    require(len(events) == len(wanted) and {n for n, _ in events} == set(wanted)
            and all(state == "ok" for _, state in events), "missing/duplicate/failed P02 named result")
    require(len(result["partitions"]) == 1 and result["partitions"][0]["ignored"] == result["partitions"][0]["failed"] == 0,
            "image partition failed/ignored/replayed")
    pattern = r"P02_(RUNTIME|WORKSPACE)_FIXTURE nonce=([0-9a-f]{64}) pid=([1-9][0-9]*) image=([A-Za-z_0-9.-]+) label=([A-Za-z_0-9-]+) root=([^\r\n]+)"
    markers = re.findall(pattern, raw)
    require(len(markers) == len(re.findall(r"P02_(?:RUNTIME|WORKSPACE)_FIXTURE", raw)), "malformed fixture marker")
    expected = {}
    if key == "ffi":
        expected["RUNTIME"] = _fixture_labels(root, RUNTIME)
    if key in ("ffi", "workspace"):
        expected["WORKSPACE"] = _fixture_labels(root, WORKSPACE)
    observed = {}; pids = set(); roots = []
    for channel, nonce, pid, stem, label, path in markers:
        require(nonce == environment["P02_RUN_NONCE"] and channel in expected and label in expected[channel], "foreign nonce/channel/label")
        require(stem == Path(image["binary"]["path"]).stem, "wrong fixture image")
        require(expected_pid is None or (type(expected_pid) is int and int(pid) == expected_pid), "wrong child PID")
        base = Path(environment["P02_" + channel + "_FIXTURES"])
        case = base / stem / pid / label
        require(Path(path) == case and str(case) == path, "foreign fixture root")
        owned(case, base); directory_binding(case)
        require((channel, label) not in observed, "replayed fixture")
        observed[channel, label] = path; pids.add(int(pid)); roots.append(path)
    require(set(observed) == {(k, label) for k, labels in expected.items() for label in labels}, "zero/missing fixture")
    require((not expected and not pids) or len(pids) == 1, "one actual process per image required")
    opens = re.findall(r"P02_WORKSPACE_OPEN nonce=([0-9a-f]{64}) root=([^\r\n]+)", raw)
    require(len(opens) == len(re.findall(r"P02_WORKSPACE_OPEN", raw)), "malformed workspace-open marker")
    wanted_opens = Counter((environment["P02_RUN_NONCE"], path) for (channel, _), path in observed.items() if channel == "WORKSPACE")
    if "WORKSPACE" in expected:
        wanted_opens[environment["P02_RUN_NONCE"], observed["WORKSPACE", "w04-aba"]] += 1
    require(Counter(opens) == wanted_opens, "missing/foreign workspace-open nonce/root")
    recovery_roots = []
    if key == "ffi" and platform_contract()["persistent_recovery_positive"]:
        # The pinned m03 test deliberately keeps its normal recovery roundtrip
        # in a sibling store. This is required, not an arbitrary extra root.
        recovery = Path(observed["RUNTIME", "m03"]).with_name("m03-recovery")
        owned(recovery, Path(environment["P02_RUNTIME_FIXTURES"])); directory_binding(recovery)
        cohorts = [p for p in recovery.iterdir() if re.fullmatch(r"cohort-[0-9a-f]{32}", p.name)]
        require(len(cohorts) == 1 and {p.name for p in recovery.iterdir()} == {"current", "writer.lock", cohorts[0].name},
                "missing/foreign m03 recovery store")
        require({p.name for p in cohorts[0].iterdir()} == {"checkpoint-1.rec", "journal-1.rec", "runtime-head.rec", "runtime-edits.rec", "runtime-state.bin"},
                "missing/foreign m03 recovery records")
        require((recovery / "writer.lock").stat().st_size == 0
                and all(p.stat().st_size > 0 for p in [recovery / "current", *cohorts[0].iterdir()]), "zero recovery record")
        recovery_roots.append(str(recovery))
    return {"role": key, "binary": image["binary"], "method_ids": wanted, "fixture_roots": sorted(roots),
        "recovery_roots": recovery_roots,
        "platform_contract": platform_contract(),
        "fixture_pid": next(iter(pids)) if pids else None,
        "discovery": {k: binding(discovery[k]["log"]["path"]) for k in ("all", "ignored")}}


def _inventory(directory, observations):
    directory = Path(directory)
    require({p.name for p in directory.iterdir()} == {"NAMESPACE.json", "runtime", "workspace"}, "unknown namespace entry")
    wanted = {Path(p) for row in observations for p in [*row["fixture_roots"], *row["recovery_roots"]]}
    actual = set()
    for channel in ("runtime", "workspace"):
        channel_root = directory / channel
        require({p for p in channel_root.iterdir()} == {p.parent.parent for p in wanted if p.is_relative_to(channel_root)}, "foreign/empty image fixture")
        for image in (directory / channel).iterdir():
            directory_binding(owned(image, directory))
            require(set(image.iterdir()) == {p.parent for p in wanted if p.is_relative_to(image)}, "foreign/empty PID fixture")
            for pid in image.iterdir():
                directory_binding(owned(pid, directory))
                for case in pid.iterdir():
                    directory_binding(owned(case, directory)); actual.add(case)
    require(actual == wanted, "missing/foreign physical fixture")
    files, directories, total = {}, {}, 0
    for base, ds, ns in os.walk(directory, followlinks=False):
        for name in sorted(ds + ns):
            path = owned(Path(base) / name, directory); rel = path.relative_to(directory).as_posix(); value = path.lstat()
            require(len(path.relative_to(directory).parts) <= 16 and len(files) + len(directories) < 2048, "fixture node quota")
            if stat.S_ISDIR(value.st_mode):
                directories[rel] = directory_binding(path)
            else:
                require(stat.S_ISREG(value.st_mode) and value.st_nlink == 1, "nonregular/hardlink fixture")
                total += value.st_size
                require(total <= 32 * 1024 * 1024 and value.st_size <= 4 * 1024 * 1024, "fixture byte quota")
                files[rel] = binding(path)
    return {"files": files, "directories": directories, "bytes": total}


def observe(root, namespace, directory, phase, context, command, parent, observations, binary=None, capture=False):
    require(type(namespace) is dict and set(namespace) == {"spec", "metadata", "directories", "environment"} | (set() if capture else {"inventory"}), "namespace fields")
    directory = Path(directory).absolute()
    spec = _spec(root, directory, phase, context, command, parent, binary)
    env = dict(zip(ENV, (str(directory / "runtime"), str(directory / "workspace"), spec["nonce"])))
    require(same(namespace["environment"], env) and same(namespace["spec"], spec), "wrong source/tool/binary/nonce/parent")
    require(same(namespace["directories"], [directory_binding(owned(directory / n, directory), True) for n in (".", "runtime", "workspace")]), "namespace object replaced")
    marker = owned(directory / "NAMESPACE.json", directory)
    require(_mode(marker) == (0o666 if os.name == "nt" else 0o600) and marker.lstat().st_nlink == 1
            and same(namespace["metadata"], binding(marker)) and same(read(marker), spec), "namespace marker changed")
    require(observations and len({r["binary"]["path"] for r in observations}) == len(observations), "zero/duplicate image observations")
    inventory = _inventory(directory, observations)
    path = directory.with_name(directory.name + ".inventory.json")
    if capture:
        write(path, inventory)
    owned(path, directory.parent)
    reference = binding(path)
    require(_mode(path) == (0o666 if os.name == "nt" else 0o600) and path.lstat().st_nlink == 1, "inventory file mode/type")
    require((capture or same(namespace["inventory"], reference)) and same(read(path), inventory), "stale fixture inventory")
    return {**namespace, "inventory": reference}


def cargo_context(root, record, command, run_id, source, tool):
    require(record.get("name") == "rust-workspace" and same(record.get("command"), command)
            and record.get("status") == "passed" and type(record.get("exit_code")) is int
            and record["exit_code"] == 0 and record.get("timed_out") is False, "workspace command/result")
    require(type(record.get("child_pid")) is int and record["child_pid"] > 0, "Cargo child identity")
    require(record.get("log", {}).get("sha256") == binding(record["log"]["path"])["sha256"], "stale Cargo log")
    return {"verification_run_id": run_id, "source_sha256": source, "tool": tool,
        "log": binding(record["log"]["path"]), "command": command, "cwd": str(Path(root).resolve()),
        "cargo_pid": record["child_pid"]}


def cargo_images(root, context, target):
    images = []
    require("--message-format=json" in context["command"] and "--show-output" in context["command"], "Cargo JSON/fixture output required")
    for line in text(context["log"]["path"]).splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if type(value) is not dict or value.get("reason") != "compiler-artifact" or not value.get("executable") or value.get("profile", {}).get("test") is not True:
            continue
        if not any(Path(value["target"]["src_path"]).resolve() == (Path(root) / n).resolve() for n in ROLES.values()):
            continue
        binary = owned(Path(value["executable"]), Path(target))
        require(type(value.get("fresh")) is bool and value["fresh"] is False, "fresh current product image required")
        images.append({"target": value["target"]["name"], "src_path": value["target"]["src_path"], "fresh": False, "binary": binding(binary)})
    require(len(images) == 3 and {role(root, image) for image in images} == set(ROLES), "missing/duplicate/foreign Cargo images")
    return sorted(images, key=lambda image: role(root, image))


def cargo_raw(context, images):
    output = {}; current = None
    def displays(image):
        path = Path(image["binary"]["path"]); cwd = Path(context["cwd"])
        names = {str(path)}
        if path.is_relative_to(cwd):
            relative = path.relative_to(cwd)
            names.update((str(relative), relative.as_posix()))
        return names
    for line in text(context["log"]["path"]).splitlines(keepends=True):
        clean = re.sub(r"\x1b\[[0-9;]*m", "", line).rstrip("\r\n")
        if re.match(r"^\s*Running ", clean):
            matches = [image["binary"]["path"] for image in images
                if any(clean.endswith("(" + display + ")") for display in displays(image))]
            require(len(matches) <= 1, "ambiguous Cargo image header")
            current = matches[0] if matches else None
            if current:
                require(current not in output, "replayed Cargo image")
                output[current] = ""
        elif re.match(r"^\s*Doc-tests ", clean):
            current = None
        if current:
            output[current] += clean + "\n"
        elif re.search(r"P02_(?:RUNTIME|WORKSPACE)_FIXTURE", clean):
            raise ValueError("P02 fixture outside its Cargo image")
    require(set(output) == {image["binary"]["path"] for image in images}, "missing actual Cargo image headers")
    return output


def cargo_proof(root, namespace, record, command, run_id, source, target, proof_path, tool, execute=None, env=None, p01_proof=None):
    """Capture or revalidate the same actual Cargo results; never replay methods."""
    root, proof_path = Path(root), Path(proof_path)
    context = cargo_context(root, record, command, run_id, source, tool)
    images = cargo_images(root, context, target); raws = cargo_raw(context, images)
    parent = {"kind": "workspace-cargo", "cwd": str(root.resolve()), "command": command}
    directory = Path(record["log"]["path"]).parent / ("p02-cargo-" + run_id)
    capture = execute is not None
    stored = None if capture else read(proof_path)
    if not capture:
        require(type(stored) is dict and set(stored) == {"schema", "context", "adapter", "namespace", "images", "discovery", "discovery_origin", "observations", "source_after"}
                and type(stored["schema"]) is int and stored["schema"] == 2, "proof schema/fields")
    discovery = []
    if p01_proof is not None:
        proof = read(p01_proof)
        require(proof.get("context", {}).get("verification_run_id") == run_id
                and proof["context"].get("source_sha256") == source
                and proof["context"].get("cwd") == str(root.resolve())
                and same(proof["context"].get("command"), command)
                and same(proof["context"].get("log"), context["log"]), "foreign P01 discovery parent")
        by_binary = {image["binary"]["path"]: pair for image, pair in zip(proof["images"], proof["discovery"])}
        discovery = [by_binary[image["binary"]["path"]] for image in images]
        origin = {"kind": "p01", "proof": binding(p01_proof)}
    else:
        scratch = directory.with_name(directory.name + "-discovery")
        if capture:
            scratch.mkdir(mode=0o700)
        origin = {"kind": "p02", "scratch": directory_binding(owned(scratch, directory.parent), True)}
    if capture:
        if p01_proof is None:
            clean_env = dict(env)
            for name in ENV:
                clean_env.pop(name, None)
            for index, image in enumerate(images):
                pair = {}
                for key, ignored in (("all", False), ("ignored", True)):
                    argv = [image["binary"]["path"], *(["--ignored"] if ignored else []), "--list", "--format", "terse"]
                    row = execute("p02-discovery-" + str(index) + "-" + key, argv, root, scratch / (str(index) + "-" + key + ".log"), 10, env=clean_env)
                    row.update(cwd=str(root.resolve()), argc=len(argv), requested_timeout=10,
                        context_binding={"phase": row["name"], "verification_run_id": run_id, "source_sha256": source,
                            "binary": image["binary"], "cargo_log": context["log"], "nonce": namespace["spec"]["nonce"]})
                    pair[key] = row
                discovery.append(pair)
    else:
        if p01_proof is not None:
            require(same(stored["discovery"], discovery), "P01 discovery partition changed")
        discovery = stored["discovery"]
        require(type(discovery) is list and len(discovery) == len(images), "missing discovery partitions")
        require(same(stored["discovery_origin"], origin), "discovery parent/root changed")
    if p01_proof is None:
        require(len(discovery) == len(images), "missing owned discovery")
        for index, (image, pair) in enumerate(zip(images, discovery)):
            require(type(pair) is dict and set(pair) == {"all", "ignored"}, "discovery roles")
            for key, row in pair.items():
                require(type(row) is dict and type(row.get("log")) is dict, "discovery record/log shape")
                name = "p02-discovery-" + str(index) + "-" + key
                expected = {"phase": name, "verification_run_id": run_id, "source_sha256": source,
                    "binary": image["binary"], "cargo_log": context["log"], "nonce": namespace["spec"]["nonce"]}
                require(row.get("name") == name and same(row.get("context_binding"), expected)
                        and row.get("cwd") == str(root.resolve()) and type(row.get("child_pid")) is int and row["child_pid"] > 0
                        and type(row.get("requested_timeout")) is int and row["requested_timeout"] == 10
                        and type(row.get("duration_ms")) is int and 0 <= row["duration_ms"] <= 10000
                        and type(row.get("argc")) is int and row["argc"] == (5 if key == "ignored" else 4), "discovery run/source/image/parent")
                require(row["log"]["path"] == str(scratch / (str(index) + "-" + key + ".log")), "foreign discovery log partition")
                owned(row["log"]["path"], scratch)
    observations = [image_observation(root, image, pair, raws[image["binary"]["path"]], namespace["environment"])
                    for image, pair in zip(images, discovery)]
    namespace = observe(root, namespace, directory, "cargo", context, command, parent, observations, capture=capture)
    proof = {"schema": 2, "context": context, "adapter": binding(Path(__file__)), "namespace": namespace,
        "discovery_origin": origin,
        "images": images, "discovery": discovery, "observations": observations, "source_after": catalog(root)}
    if capture:
        write(proof_path, proof)
    else:
        require(same(stored, proof), "stale source/image/discovery/parent/proof")
    return namespace


def validate(root, proof_path, record, command, run_id, source, target, tool, p01_proof=None):
    before = binding(proof_path); proof = read(proof_path)
    require(same(record.get("p02_namespace"), proof["namespace"]), "workspace namespace omitted/changed")
    cargo_proof(root, proof["namespace"], record, command, run_id, source, target, proof_path, tool, p01_proof=p01_proof)
    require(same(before, binding(proof_path)), "proof changed during readback")
    return {"images": 3, "P02_named_results": 107, "proof_sha256": before["sha256"]}


def registered_proof(qualification, record, proof_path, output):
    """Bind the last nested readback to the exact already-registered proof bytes."""
    artifacts = [row for row in record.get("artifacts", [])
        if row.get("kind") == "p02_cohort_proof" and output(row["path"]).resolve() == Path(proof_path).resolve()]
    require(len(artifacts) == 1 and qualification["proof_sha256"] == artifacts[0]["sha256"],
            "P02 proof differs from registered artifact")
