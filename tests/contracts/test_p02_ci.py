"""Inert Python evidence models. These tests never compile or execute Rust."""
from copy import deepcopy
from contextlib import nullcontext
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def module(name):
    spec = importlib.util.spec_from_file_location("test_p02_" + name, ROOT / "integration/registry" / (name + ".py"))
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value


RUNTIME_LABELS = [*("r%02d" % n for n in range(1, 9)), "r06-workspace-reopen",
    *("c%02d" % n for n in range(1, 9)), "c05-drop", *("m%02d" % n for n in range(1, 11))]
WORKSPACE_LABELS = ["w01-fresh", "w02-two", "w03-stale", "w04-aba", "w05-source", "w05-foreign",
    "w06-unicode", "w07-quota", "w08-save-undo"]


def runtime_labels():
    labels = [*RUNTIME_LABELS, "m03-memory"]
    if sys.platform != "darwin":
        labels.remove("m03"); labels.append("m03-unconfirmed")
    return labels


class Model:
    """Real source bytes plus synthetic inert images/logs; no Rust acceptance."""
    def __init__(self, test, verifier=None):
        self.v = verifier; self.a = verifier.p02_ci if verifier else module("p02_ci")
        base = Path(os.environ.get("P02_CI_TEST_FIXTURE", ROOT / ".local/p02-ci-python-fixtures"))
        base.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(prefix="inert-evidence-", dir=base)).resolve()
        test.addCleanup(self.cleanup_tree, self.root)
        for name in self.a.SOURCE_PINS:
            target = self.root / name; target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / name).read_bytes()); target.chmod(0o644)
        self.target = self.root / "target"; self.target.mkdir()
        self.logs = self.root / ".local/run"; self.logs.mkdir(parents=True, mode=0o700)
        self.tool = self.target / ("inert-cargo.exe" if os.name == "nt" else "inert-cargo")
        self.tool.write_bytes(b"INERT MODEL DATA; THIS IS NEVER EXECUTED"); self.tool.chmod(0o700)
        self.context = {"verification_run_id": "13" * 16, "source_sha256": "29" * 32, "tool": self.a.binding(self.tool)}
        self.command = [str(self.tool), "test", "--workspace", "--locked", "--target-dir", str(self.target),
                        "--message-format=json", "--", "--show-output"]
        self.parent = {"kind": "workspace-cargo", "cwd": str(self.root), "command": self.command}
        self.directory = self.logs / ("p02-cargo-" + self.context["verification_run_id"])
        self.images = []; self.discoveries = {}; self.raws = {}; self.calls = []; self.environments = []; self.relative_headers = False
        wanted = self.a.methods(self.root)
        for key, source in self.a.ROLES.items():
            binary = self.target / ("inert-" + key + "-abcd" + (".exe" if os.name == "nt" else ""))
            binary.write_bytes(b"INERT NONEXECUTED MODEL IMAGE " + key.encode()); binary.chmod(0o700)
            image = {"target": key, "src_path": str(self.root / source), "fresh": False, "binary": self.a.binding(binary)}
            self.images.append(image)
            pair = {}
            for selection in ("all", "ignored"):
                ids = wanted[key] if selection == "all" else []
                path = self.logs / (key + "-" + selection + ".log")
                path.write_text("".join(name + ": test\n" for name in ids), newline="\n")
                pair[selection] = self.record("discovery", [str(binary), *(["--ignored"] if selection == "ignored" else []),
                    "--list", "--format", "terse"], path)
            self.discoveries[key] = pair

    @staticmethod
    def cleanup_tree(root):
        def retry_readonly(function, path, error):
            if not isinstance(error[1], PermissionError):
                raise error[1]
            # Windows cannot unlink a read-only marker. Only this model's
            # newly created disposable tree is eligible for this retry.
            Path(path).chmod(0o700 if Path(path).is_dir() else 0o600)
            function(path)
        shutil.rmtree(root, onerror=retry_readonly)

    def record(self, name, command, path):
        return {"name": name, "command": command, "status": "passed", "exit_code": 0, "timed_out": False,
            "duration_ms": 1, "child_pid": 777, "reason": None, "artifact_sha256": None,
            "log": {"kind": "command_log", "path": str(path), "sha256": self.a.binding(path)["sha256"]}}

    def namespace(self):
        return self.a.create(self.root, self.directory, "cargo", self.context, self.command, self.parent)

    def outputs(self, namespace, images=None):
        self.observations = []
        wanted = self.a.methods(self.root)
        for index, image in enumerate(self.images if images is None else images):
            key = self.a.role(self.root, image); pid = 810 + index; stem = Path(image["binary"]["path"]).stem
            raw = "running " + str(len(wanted[key])) + " tests\n"
            raw += "".join("test " + name + " ... ok\n" for name in wanted[key])
            channels = []
            if key == "ffi": channels.append(("RUNTIME", runtime_labels()))
            if key in ("ffi", "workspace"): channels.append(("WORKSPACE", WORKSPACE_LABELS))
            for channel, labels in channels:
                base = Path(namespace["environment"]["P02_" + channel + "_FIXTURES"])
                for label in labels:
                    case = base / stem / str(pid) / label; case.mkdir(parents=True)
                    (case / "main.tex").write_bytes(b"INERT PYTHON FIXTURE")
                    raw += f"P02_{channel}_FIXTURE nonce={namespace['environment']['P02_RUN_NONCE']} pid={pid} image={stem} label={label} root={case}\n"
                    if channel == "WORKSPACE":
                        raw += f"P02_WORKSPACE_OPEN nonce={namespace['environment']['P02_RUN_NONCE']} root={case}\n" * (2 if label == "w04-aba" else 1)
            if key == "ffi" and sys.platform == "darwin":
                recovery = Path(namespace["environment"]["P02_RUNTIME_FIXTURES"]) / stem / str(pid) / "m03-recovery"
                cohort = recovery / ("cohort-" + "a1" * 16); cohort.mkdir(parents=True)
                (recovery / "writer.lock").write_bytes(b""); (recovery / "current").write_bytes(b"INERT MODEL SELECTOR")
                for name in ["checkpoint-1.rec", "journal-1.rec", "runtime-head.rec", "runtime-edits.rec", "runtime-state.bin"]:
                    (cohort / name).write_bytes(b"INERT MODEL RECOVERY RECORD")
            raw += f"test result: ok. {len(wanted[key])} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
            self.raws[key] = raw
            self.observations.append(self.a.image_observation(self.root, image, self.discoveries[key], raw, namespace["environment"], pid))
        return self.observations

    def seal(self):
        namespace = self.namespace(); self.outputs(namespace)
        return self.a.observe(self.root, namespace, self.directory, "cargo", self.context, self.command,
            self.parent, self.observations, capture=True)

    def reobserve(self, namespace, **changes):
        values = dict(root=self.root, namespace=namespace, directory=self.directory, phase="cargo", context=self.context,
            command=self.command, parent=self.parent, observations=self.observations)
        values.update(changes)
        return self.a.observe(**values)

    def execute(self, name, argv, cwd, log, timeout, env=None):
        self.calls.append(list(argv)); self.environments.append(dict(env or {})); log = Path(log)
        if argv == self.command:
            self.context["verification_run_id"] = env["P00_VERIFICATION_RUN_ID"]
            namespace = {"environment": {key: env[key] for key in self.a.ENV}}
            self.outputs(namespace)
            raw = ""
            for image in self.images:
                raw += json.dumps({"reason": "compiler-artifact", "target": {"name": image["target"], "src_path": image["src_path"]},
                    "profile": {"test": True}, "fresh": False, "executable": image["binary"]["path"]}) + "\n"
            for image in self.images:
                display = str(Path(image["binary"]["path"]).relative_to(self.root)) if self.relative_headers else image["binary"]["path"]
                raw += "     Running tests.rs (" + display + ")\n" + self.raws[self.a.role(self.root, image)]
        else:
            image = next(image for image in self.images if image["binary"]["path"] == argv[0])
            key = self.a.role(self.root, image)
            raw = "" if "--ignored" in argv else "".join(name + ": test\n" for name in self.a.methods(self.root)[key])
        log.write_text(raw, newline="\n")
        return self.record(name, list(argv), log)


class P02Fixtures(unittest.TestCase):
    def test_windows_filename_execute_view_preserves_every_object_guard(self):
        a = module("p02_ci")
        facts = dict(st_dev=7, st_ino=41, st_size=11, st_mode=0o100777, st_uid=3, st_nlink=1, st_mtime_ns=97)
        before = SimpleNamespace(**facts); opened = SimpleNamespace(**{**facts, "st_mode": 0o100666})
        a._stable_file_stats(Path("inert.EXE"), before, opened, before, True)
        for key in ("st_dev", "st_ino", "st_size", "st_uid", "st_nlink", "st_mtime_ns"):
            wrong = SimpleNamespace(**{**vars(opened), key: getattr(opened, key) + 1})
            with self.subTest(field=key), self.assertRaises(ValueError):
                a._stable_file_stats(Path("inert.exe"), before, wrong, before, True)
        changed = SimpleNamespace(**{**facts, "st_mode": 0o100555})
        with self.assertRaises(ValueError): a._stable_file_stats(Path("inert.exe"), before, opened, changed, True)
        with self.assertRaises(ValueError): a._stable_file_stats(Path("inert.py"), before, opened, before, True)
        with self.assertRaises(ValueError): a._stable_file_stats(Path("inert.exe"), before, opened, before, False)

    def test_real_executable_suffix_binding_uses_path_mode_and_exact_bytes(self):
        m = Model(self); path = m.root / "inert-nonexecuted.EXE"
        path.write_bytes(b"INERT NONEXECUTED EXECUTABLE-SUFFIX FIXTURE"); path.chmod(0o700)
        value = m.a.binding(path)
        self.assertEqual(value["sha256"], m.a.binding(path)["sha256"])
        self.assertEqual(value["mode"], m.a._mode(path))

    def test_catalog_is_complete_and_absence_is_not_partial_success(self):
        m = Model(self)
        self.assertEqual(len(m.a.catalog(m.root)), 16)
        (m.root / m.a.NEW_PATHS[0]).unlink()
        with self.assertRaises((ValueError, OSError)): m.a.catalog(m.root)
        self.assertIsNone(m.a.catalog(m.target))

    def test_source_pin_rejects_changed_product_bytes(self):
        m = Model(self); path = m.root / m.a.WORKSPACE; path.write_bytes(path.read_bytes() + b"\n// foreign\n")
        with self.assertRaisesRegex(ValueError, "source pin"): m.a.catalog(m.root)

    def test_model_positive_has_three_images_107_named_results_and_47_roots(self):
        m = Model(self); namespace = m.seal()
        self.assertEqual([len(row["method_ids"]) for row in m.observations], [32, 8, 67])
        self.assertEqual(sum(len(row["fixture_roots"]) for row in m.observations), 47)
        self.assertEqual(sum(len(row["recovery_roots"]) for row in m.observations), int(sys.platform == "darwin"))
        self.assertEqual(m.reobserve(namespace), namespace)

    def test_stale_directory_cannot_be_reused(self):
        m = Model(self); m.namespace()
        with self.assertRaises(FileExistsError): m.namespace()

    def test_missing_zero_and_foreign_observations_refuse(self):
        m = Model(self); namespace = m.seal()
        for value in [[], [m.observations[0], m.observations[0]]]:
            with self.subTest(value=len(value)), self.assertRaises(ValueError): m.reobserve(namespace, observations=value)
        foreign = m.directory / "workspace/foreign-image/123/foreign-case"; foreign.mkdir(parents=True)
        with self.assertRaises(ValueError): m.reobserve(namespace)

    def test_missing_environment_and_wrong_nonce_refuse(self):
        m = Model(self); namespace = m.seal()
        for key in m.a.ENV:
            value = deepcopy(namespace); value["environment"].pop(key)
            with self.subTest(key=key), self.assertRaises(ValueError): m.reobserve(value)
        value = deepcopy(namespace); value["environment"]["P02_RUN_NONCE"] = "00" * 32
        with self.assertRaises(ValueError): m.reobserve(value)

    def test_wrong_source_tool_command_and_parent_refuse(self):
        m = Model(self); namespace = m.seal()
        for key, value in [("source_sha256", "42" * 32), ("verification_run_id", "54" * 16)]:
            context = deepcopy(m.context); context[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): m.reobserve(namespace, context=context)
        with self.assertRaises(ValueError): m.reobserve(namespace, command=m.command + ["--ignored"])
        with self.assertRaises(ValueError): m.reobserve(namespace, parent={**m.parent, "cwd": str(m.target)})
        m.tool.write_bytes(b"CHANGED INERT TOOL")
        with self.assertRaises(ValueError): m.reobserve(namespace)

    def test_missing_duplicate_failed_and_unknown_named_results_refuse(self):
        m = Model(self); namespace = m.namespace(); m.outputs(namespace)
        image = m.images[-1]; raw = m.raws["ffi"]; method = m.a.methods(m.root)["ffi"][0]
        for bad in [raw.replace("test " + method + " ... ok\n", ""), raw.replace(" ... ok", " ... FAILED", 1),
                    raw.replace("test " + method, "test foreign_method", 1), raw + "test foreign_method ... ok\n"]:
            with self.subTest(raw=bad[:40]), self.assertRaises(ValueError):
                m.a.image_observation(m.root, image, m.discoveries["ffi"], bad, namespace["environment"])

    def test_wrong_fixture_nonce_image_pid_root_and_workspace_open_refuse(self):
        m = Model(self); namespace = m.namespace(); m.outputs(namespace)
        image = m.images[-1]; raw = m.raws["ffi"]
        mutations = [raw.replace(namespace["environment"]["P02_RUN_NONCE"], "00" * 32, 1),
            raw.replace("image=inert-ffi-abcd", "image=foreign"), raw.replace("pid=812", "pid=123"),
            raw.replace("label=r01 root=", "label=foreign root="), raw.replace("P02_WORKSPACE_OPEN nonce=", "P02_WORKSPACE_OPEN nonce=BAD", 1)]
        for bad in mutations:
            with self.subTest(raw=bad[:60]), self.assertRaises((ValueError, OSError)):
                m.a.image_observation(m.root, image, m.discoveries["ffi"], bad, namespace["environment"], 812)
        with self.assertRaises(ValueError):
            m.a.image_observation(m.root, image, m.discoveries["ffi"], raw, namespace["environment"], 999)

    def test_zero_stale_ignored_and_foreign_discovery_refuse(self):
        m = Model(self); namespace = m.namespace(); m.outputs(namespace)
        image = m.images[1]; pair = m.discoveries["workspace"]
        for key, raw in [("all", ""), ("all", "foreign: test\n"), ("ignored", m.a.methods(m.root)["workspace"][0] + ": test\n")]:
            value = deepcopy(pair); path = m.logs / ("mutated-" + key + ".log"); path.write_text(raw)
            value[key]["log"] = {"kind": "command_log", "path": str(path), "sha256": m.a.binding(path)["sha256"]}
            with self.subTest(key=key, raw=raw), self.assertRaises(ValueError):
                m.a.image_observation(m.root, image, value, m.raws["workspace"], namespace["environment"])
        Path(pair["all"]["log"]["path"]).write_text("changed")
        with self.assertRaises(ValueError):
            m.a.image_observation(m.root, image, pair, m.raws["workspace"], namespace["environment"])

    def test_wrong_and_replaced_image_refuse(self):
        m = Model(self); namespace = m.namespace(); m.outputs(namespace)
        image = deepcopy(m.images[1]); image["src_path"] = str(m.root / m.a.CANONICAL)
        with self.assertRaises(ValueError):
            m.a.image_observation(m.root, image, m.discoveries["workspace"], m.raws["workspace"], namespace["environment"])
        Path(m.images[1]["binary"]["path"]).write_bytes(b"REPLACED IMAGE")
        with self.assertRaises(ValueError):
            m.a.image_observation(m.root, m.images[1], m.discoveries["workspace"], m.raws["workspace"], namespace["environment"])

    def test_late_fixture_and_inventory_mutation_refuse(self):
        m = Model(self); namespace = m.seal()
        case = Path(m.observations[1]["fixture_roots"][0]); (case / "main.tex").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "inventory"): m.reobserve(namespace)
        # A fresh positive control makes the manifest mutation independent.
        m = Model(self); namespace = m.seal(); self.assertEqual(m.reobserve(namespace), namespace)
        Path(namespace["inventory"]["path"]).write_text("{}")
        with self.assertRaises(ValueError): m.reobserve(namespace)

    def test_inventory_reformat_during_nested_readback_refuses(self):
        m = Model(self); namespace = m.seal()
        self.assertEqual(m.reobserve(namespace), namespace)
        manifest = Path(namespace["inventory"]["path"]); original_read = m.a.read
        def reformat_after_read(path, *args, **kwargs):
            data = original_read(path, *args, **kwargs)
            if Path(path) == manifest:
                manifest.write_text(json.dumps(data, indent=2))
            return data
        with patch.object(m.a, "read", side_effect=reformat_after_read):
            with self.assertRaisesRegex(ValueError, "inventory changed during readback"):
                m.reobserve(namespace)

    def test_model_cleanup_handles_readonly_files_and_partial_setup(self):
        owner = unittest.TestCase(); m = Model(owner); root = m.root
        marker = root / "readonly-marker"; marker.write_bytes(b"inert"); marker.chmod(0o444)
        self.assertTrue(owner.doCleanups())
        self.assertFalse(root.exists())
        owner = unittest.TestCase(); created = []; original_mkdtemp = tempfile.mkdtemp
        def own_directory(*args, **kwargs):
            value = original_mkdtemp(*args, **kwargs); created.append(Path(value)); return value
        with patch.object(tempfile, "mkdtemp", side_effect=own_directory):
            with patch.object(Path, "write_bytes", side_effect=OSError("inert setup refusal")):
                with self.assertRaisesRegex(OSError, "inert setup refusal"):
                    Model(owner)
        self.assertEqual(len(created), 1)
        self.assertTrue(created[0].exists())
        self.assertTrue(owner.doCleanups())
        self.assertFalse(created[0].exists())

    def test_balanced_extra_named_result_refuses_in_each_standalone_image(self):
        m = Model(self); namespace = m.namespace(); m.outputs(namespace)
        for image in m.images:
            key = m.a.role(m.root, image)
            if key == "ffi": continue
            count = len(m.a.methods(m.root)[key])
            raw = m.raws[key].replace(f"running {count} tests", f"running {count + 1} tests")
            raw = raw.replace(f"{count} passed;", f"{count + 1} passed;")
            raw = raw.replace("test result:", "test foreign_success ... ok\ntest result:")
            with self.subTest(image=key), self.assertRaisesRegex(ValueError, "named result"):
                m.a.image_observation(m.root, image, m.discoveries[key], raw, namespace["environment"])

    def test_selection_uses_image_source_before_unqualified_method_names(self):
        m = Model(self); wanted = m.a.methods(m.root)
        foreign = deepcopy(m.images[0]); foreign["src_path"] = str(m.root / "tests/unrelated_contract.rs")
        self.assertFalse(m.a.selected(m.root, foreign, [wanted["canonical"][0]]))
        self.assertFalse(m.a.selected(m.root, foreign, wanted["canonical"]))
        for image in m.images:
            key = m.a.role(m.root, image)
            self.assertTrue(m.a.selected(m.root, image, wanted[key]))
            with self.subTest(image=key), self.assertRaisesRegex(ValueError, "partial/foreign"):
                m.a.selected(m.root, image, wanted[key][1:])
            with self.subTest(image=key), self.assertRaisesRegex(ValueError, "partial/foreign"):
                extra = m.a.FFI + "foreign" if key == "ffi" else "foreign"
                m.a.selected(m.root, image, [*wanted[key], extra])

    def test_replaced_directory_marker_modes_and_hardlinks_refuse(self):
        m = Model(self); namespace = m.seal(); marker = m.directory / "NAMESPACE.json"
        marker.chmod(0o444)
        with self.assertRaises(ValueError): m.reobserve(namespace)
        marker.chmod(0o600)
        source = Path(m.observations[1]["fixture_roots"][0]) / "main.tex"
        alias = source.with_name("alias.tex"); os.link(source, alias)
        with self.assertRaises(ValueError): m.reobserve(namespace)

    def test_symlink_or_hardlink_alias_cannot_escape_namespace(self):
        m = Model(self); namespace = m.seal(); source = Path(m.observations[1]["fixture_roots"][0]) / "main.tex"
        alias = source.with_name("outside.tex")
        if os.name == "nt": os.link(m.tool, alias)
        else: alias.symlink_to(m.tool)
        with self.assertRaises(ValueError): m.reobserve(namespace)

    def test_empty_foreign_image_directory_refuses(self):
        m = Model(self); namespace = m.seal(); (m.directory / "runtime/foreign-empty").mkdir()
        with self.assertRaises(ValueError): m.reobserve(namespace)

    def test_required_m03_recovery_sibling_is_neither_optional_nor_foreign(self):
        m = Model(self); namespace = m.namespace(); m.outputs(namespace); image = m.images[-1]
        if sys.platform != "darwin":
            self.assertEqual(m.observations[-1]["recovery_roots"], [])
            self.assertIn("m03_recovery_refuses_unconfirmed_platform", m.a.methods(m.root)["runtime"])
            return
        root = Path(m.observations[-1]["recovery_roots"][0]); (root / "current").unlink()
        with self.assertRaisesRegex(ValueError, "m03 recovery"):
            m.a.image_observation(m.root, image, m.discoveries["ffi"], m.raws["ffi"], namespace["environment"])


class P02VerifierHooks(unittest.TestCase):
    def test_platform_catalog_keeps_mac_oracle_and_requires_other_platform_refusal(self):
        m = Model(self)
        for target in ("darwin", "linux", "win32"):
            with self.subTest(target=target), patch.object(m.a.sys, "platform", target):
                names = m.a.methods(m.root)["runtime"]
                self.assertEqual(len(names), 27)
                self.assertIn("m03_portable_common_redo_group", names)
                self.assertEqual("m03_common_redo_group" in names, target == "darwin")
                self.assertEqual("m03_recovery_refuses_unconfirmed_platform" in names, target != "darwin")
                labels = m.a._fixture_labels(m.root, m.a.RUNTIME)
                self.assertEqual("m03" in labels, target == "darwin")
                self.assertEqual("m03-unconfirmed" in labels, target != "darwin")
                self.assertIn("m03-memory", labels)

    def test_p01_normal_revalidates_its_separate_p02_namespace_and_child(self):
        p01 = module("p01_ci"); m = Model(self)
        scratch = m.logs / "p01-normal"; scratch.mkdir(mode=0o700)
        for name in ("durable", "runtime", "tmp"): (scratch / name).mkdir(mode=0o700)
        cargo_log = m.logs / "inert-cargo.log"; cargo_log.write_bytes(b"INERT MODEL CARGO LOG")
        context = {**m.context, "cwd": str(m.root), "command": m.command, "log": m.a.binding(cargo_log)}
        nonce = "37" * 32; image = m.images[1]; pair = m.discoveries["workspace"]
        argv = p01.normal_command(image, m.a.methods(m.root)["workspace"])
        parent = m.a.normal_parent(scratch, nonce, context, pair)
        namespace = m.a.create(m.root, scratch / "normal-0-p02", "normal-0", context, argv, parent, image["binary"])
        observations = m.outputs(namespace, [image])
        namespace = m.a.observe(m.root, namespace, scratch / "normal-0-p02", "normal-0", context, argv, parent,
            observations, image["binary"], capture=True)
        raw = m.raws["workspace"]; log = scratch / "normal-0.log"; log.write_text(raw)
        record = m.record("normal-0", argv, log)
        record.update(child_pid=810, argc=len(argv), requested_timeout=10, cwd=str(m.root),
            owned_environment=p01.owned_environment(scratch, nonce), p02_namespace=namespace,
            context_binding={"phase": "normal-0", "verification_run_id": context["verification_run_id"],
                "source_sha256": context["source_sha256"], "binary": image["binary"], "driver_nonce": nonce})
        with patch.object(p01, "f05_case_ids", return_value=[]):
            identities, outputs = p01.normal_observation([image], [pair], [record], scratch, nonce, context)
            self.assertEqual(len(identities), 8); self.assertEqual(outputs[0]["p02_namespace"], namespace)
            value = deepcopy(record); value["child_pid"] = 999
            with self.assertRaises(ValueError): p01.normal_observation([image], [pair], [value], scratch, nonce, context)
            value = deepcopy(record); value["p02_namespace"]["spec"]["parent"]["driver_nonce"] = "00" * 32
            with self.assertRaises(ValueError): p01.normal_observation([image], [pair], [value], scratch, nonce, context)
            value = deepcopy(record); value.pop("p02_namespace")
            with self.assertRaises((ValueError, KeyError)): p01.normal_observation([image], [pair], [value], scratch, nonce, context)

    def test_shared_profiles_require_proof_json_and_fixture_output_on_all_ci_platforms(self):
        v = module("verify")
        for platform, machine in [("darwin", "arm64"), ("linux", "x86_64"), ("win32", "AMD64")]:
            with self.subTest(platform=platform):
                args = SimpleNamespace(profile="shared", native_schema=False, cargo_executable=None, offline=True,
                    run_dir=ROOT / ".local/inert-p02-plan")
                # This is an inert cross-platform plan, not native Mac execution.
                # Model the tracked POSIX source mode only at the stat boundary;
                # production P01's strict mode/hash checks remain unchanged.
                original_mode = v.p01_ci._mode
                def virtual_mac_source_mode(path):
                    actual = original_mode(path)
                    return 0o644 if Path(path).suffix == ".rs" and actual == 0o666 else actual
                boundary = patch.object(v.p01_ci, "_mode", virtual_mac_source_mode) if os.name == "nt" and platform == "darwin" else nullcontext()
                with boundary:
                    steps = v.build_steps(ROOT, args, platform, machine)
                rust = next(step for step in steps if step["name"] == "rust-workspace")
                clean = next(step for step in steps if step["name"] == "runtime-clean")
                self.assertLess(steps.index(clean), steps.index(rust))
                self.assertIn("runtime-clean", rust["depends_on"])
                self.assertEqual(clean["argv"].count("-p"), 6)
                self.assertIn("--message-format=json", rust["argv"]); self.assertIn("--show-output", rust["argv"])
                self.assertIn("p02_cohort_proof", rust["required_artifact_kinds"])
                self.assertIn("p01_rust_build_log" if "p01_cohort" in rust else "p02_rust_build_log", rust["required_artifact_kinds"])
                outputs = [row["path"] for row in rust["artifact_paths"]]
                self.assertEqual(len(outputs), len(set(outputs)), "shared Cargo log must be registered exactly once")
                self.assertNotIn("p02_cohort", next(s for s in steps if s["name"] == "isolated-module-seam"))

    def test_p02_paths_still_require_runtime_without_selector_policy_changes(self):
        gate = module("ci_gate"); adapter = module("p02_ci")
        for name in [*adapter.SOURCE_PINS, "integration/registry/p02_ci.py", "tests/contracts/test_p02_ci.py"]:
            with self.subTest(path=name): self.assertTrue(gate.requires_runtime([name]))

    def run_model(self, relative_headers=False, reformat_at_final=False):
        v = module("verify"); m = Model(self, v)
        m.relative_headers = relative_headers
        proof = m.logs / "p02-fixture-cohorts.json"
        step = {"name": "rust-workspace", "kind": "rust-unit", "summary_format": "rust", "argv": m.command,
            "p02_cohort": {"proof": str(proof.relative_to(m.root))},
            "artifact_paths": [{"path": str(proof.relative_to(m.root)), "kind": "p02_cohort_proof"}],
            "required_artifact_kinds": ["p02_cohort_proof"]}
        original_validate = v.p02_ci.validate
        def late_reformat(*args, **kwargs):
            path = Path(args[1]); path.write_text(json.dumps(m.a.read(path), indent=2))
            return original_validate(*args, **kwargs)
        final = patch.object(v.p02_ci, "validate", side_effect=late_reformat) if reformat_at_final else nullcontext()
        with patch.object(v, "execute_step", m.execute), final:
            report = v.run_checks(m.root, list(m.a.SOURCE_PINS), [step], m.logs, 10)
        report["selection"] = {"platform": sys.platform, "machine": __import__("platform").machine()}
        return v, m, step, report

    def test_actual_verifier_provisions_then_revalidates_inert_cargo_model(self):
        v, m, step, report = self.run_model()
        self.assertEqual(report["status"], "passed", report["checks"])
        row = report["checks"][0]
        self.assertEqual(row["test_summary"]["passed"], 107)
        self.assertEqual(len(m.calls), 7)  # One inert Cargo call plus six discovery calls; no method replay.
        self.assertEqual(set(m.a.ENV), set(m.environments[0]) & set(m.a.ENV))
        self.assertTrue(all(not (set(env) & set(m.a.ENV)) for env in m.environments[1:]))
        current = v.source_identity(m.root, list(m.a.SOURCE_PINS))
        result = v.evidence.revalidate_report(m.root, report, current, [step])
        self.assertEqual(result["status"], "current", result)

    def test_cargo_checkout_relative_image_headers_bind_the_same_exact_binary(self):
        v, m, step, report = self.run_model(relative_headers=True)
        self.assertEqual(report["status"], "passed", report["checks"])
        current = v.source_identity(m.root, list(m.a.SOURCE_PINS))
        self.assertEqual(v.evidence.revalidate_report(m.root, report, current, [step])["status"], "current")
        context = m.a.read(m.root / step["p02_cohort"]["proof"])["context"]
        images = m.a.cargo_images(m.root, context, m.target)
        raw = m.a.text(context["log"]["path"])
        path = Path(context["log"]["path"])
        # A different relative parent cannot be resolved as the captured image.
        path.write_text(raw.replace("(target/", "(foreign/target/").replace("(target\\", "(foreign\\target\\"))
        with self.assertRaises(ValueError): m.a.cargo_raw(context, images)

    def test_read_only_report_rejects_late_nested_fixture_and_omitted_namespace(self):
        v, m, step, report = self.run_model(); self.assertEqual(report["status"], "passed", report["checks"])
        current = v.source_identity(m.root, list(m.a.SOURCE_PINS))
        value = deepcopy(report); value["checks"][0].pop("p02_namespace")
        self.assertNotEqual(v.evidence.revalidate_report(m.root, value, current, [step])["status"], "current")
        path = Path(m.observations[1]["fixture_roots"][0]) / "main.tex"; path.write_bytes(b"LATE CHANGE")
        self.assertNotEqual(v.evidence.revalidate_report(m.root, report, current, [step])["status"], "current")

    def test_direct_proof_validation_rejects_stale_discovery_run_and_foreign_parent(self):
        v, m, step, report = self.run_model(); self.assertEqual(report["status"], "passed", report["checks"])
        row = report["checks"][0]; path = m.root / step["p02_cohort"]["proof"]; original = m.a.read(path)
        for change in ["verification_run_id", "nonce", "binary"]:
            value = deepcopy(original); binding = value["discovery"][0]["all"]["context_binding"]
            binding[change] = {"path": "foreign"} if change == "binary" else "00" * (16 if change == "verification_run_id" else 32)
            path.write_text(json.dumps(value))
            with self.subTest(field=change), self.assertRaisesRegex(ValueError, "discovery run/source/image/parent"):
                m.a.validate(m.root, path, row, step["argv"], row["verification_run_id"], row["source_sha256"], m.target, row["rust_build_tool"])

    def test_warm_cargo_artifact_is_still_refused_after_shared_clean_is_planned(self):
        v, m, step, report = self.run_model(); self.assertEqual(report["status"], "passed", report["checks"])
        context = m.a.read(m.root / step["p02_cohort"]["proof"])["context"]
        log = Path(context["log"]["path"])
        raw = log.read_text(); self.assertIn('"fresh": false', raw)
        log.write_text(raw.replace('"fresh": false', '"fresh": true'))
        with self.assertRaisesRegex(ValueError, "fresh current product image"):
            m.a.cargo_images(m.root, context, m.target)

    def test_malformed_retained_discovery_rows_are_rejections_not_attribute_errors(self):
        v, m, step, report = self.run_model(); self.assertEqual(report["status"], "passed", report["checks"])
        row = report["checks"][0]; path = m.root / step["p02_cohort"]["proof"]; original = m.a.read(path)
        current = v.source_identity(m.root, list(m.a.SOURCE_PINS))
        for field in ("all", "ignored"):
            for malformed in (None, [], "foreign"):
                value = deepcopy(original); value["discovery"][0][field] = malformed
                path.write_text(json.dumps(value))
                with self.subTest(field=field, shape=type(malformed).__name__):
                    with self.assertRaisesRegex(ValueError, "discovery record/log shape"):
                        m.a.validate(m.root, path, row, step["argv"], row["verification_run_id"], row["source_sha256"], m.target, row["rust_build_tool"])
                    # Rebind only the outer artifact, forcing nested shape validation.
                    changed = deepcopy(report); check = changed["checks"][0]
                    check["artifacts"][0]["sha256"] = m.a.binding(path)["sha256"]
                    check["artifact_sha256"] = check["artifacts"][0]["sha256"]
                    result = v.evidence.revalidate_report(m.root, changed, current, [step])
                    self.assertEqual(result["status"], "invalid_test_summary", result)

    def test_run_checks_rejects_proof_reformat_after_artifact_capture(self):
        v, m, step, report = self.run_model(reformat_at_final=True)
        self.assertEqual(report["status"], "failed", report["checks"])
        self.assertEqual(report["checks"][0]["status"], "stale_artifact")
        self.assertIn("registered artifact", report["checks"][0]["reason"])

    def test_final_read_only_proof_digest_matches_registered_artifact(self):
        v, m, step, report = self.run_model(); self.assertEqual(report["status"], "passed", report["checks"])
        current = v.source_identity(m.root, list(m.a.SOURCE_PINS))
        adapter = v.evidence.p02_ci; original_validate = adapter.validate; calls = []
        def late_reformat(*args, **kwargs):
            calls.append(True)
            if len(calls) == 2:
                path = Path(args[1]); path.write_text(json.dumps(m.a.read(path), indent=2))
            return original_validate(*args, **kwargs)
        with patch.object(adapter, "validate", side_effect=late_reformat):
            result = v.evidence.revalidate_report(m.root, report, current, [step])
        self.assertEqual(len(calls), 2)
        self.assertEqual(result["status"], "stale_artifact", result)
        self.assertIn("registered artifact", result["reason"])


if __name__ == "__main__":
    unittest.main()
