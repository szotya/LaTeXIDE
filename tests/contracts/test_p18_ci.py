"""P18's required cases and compiler outputs must actually execute and validate."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import stat
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
IDS = ["os_subprocess_boundary_probe", "review_focus_r2",
       "security_service::execution::tests::compiler_adapter_real_tex_offline",
       "security_service::generation::tex_tests::generation_real_tex_offline"]
P01_NORMAL_EXCLUSIONS = [
    "durable_contract::durable_crash_child",
    "durable_contract::durable_own_child_exit_matrix_preserves_last_caller_observed_ack",
    "runtime_contract::runtime_crash_child",
    "runtime_contract::runtime_own_process_exit_restores_committed_state_save_intent_and_undo",
]
P01_SOURCE_FILES = [
    "crates/latexide-core/tests/recovery_contract.rs",
    "crates/latexide-core/src/services/recovery/durable_tests.rs",
    "crates/latexide-core/src/services/recovery/runtime_tests.rs",
]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class P18CIContract(unittest.TestCase):
    def gate(self):
        path = ROOT / "integration/registry/p18_ci.py"
        self.assertTrue(path.is_file(), "P18 CI qualification is missing")
        return load("p18_ci_contract", path)

    def log(self, ids=IDS):
        return "     Running tests/security_policy_contract.rs (target/debug/deps/security_policy_contract-123abc)\n" + "\n".join(f"test {name} ... ok" for name in ids) + f"\ntest result: ok. {len(ids)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n"

    def test_all_four_required_parent_cases_must_pass_once(self):
        gate = self.gate()
        self.assertEqual(gate.validate_workspace_log(self.log()), IDS)
        for ids in (IDS[:-1], IDS + [IDS[0]], ["another_test"]):
            with self.subTest(ids=ids), self.assertRaises(ValueError):
                gate.validate_workspace_log(self.log(ids))

    def test_ignored_failed_or_private_helper_cannot_qualify_the_workspace(self):
        gate = self.gate()
        for text in (self.log().replace("... ok", "... ignored", 1),
                     self.log().replace("... ok", "... FAILED", 1),
                     self.log(IDS + ["p18_os_probe_child"]), ""):
            with self.subTest(text=text), self.assertRaises(ValueError):
                gate.validate_workspace_log(text)

    def macos_profile(self, p01_state="absent"):
        verify = load("p18_profile_contract", ROOT / "integration/registry/verify.py")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            pins = {}
            if p01_state != "absent":
                for name in P01_SOURCE_FILES:
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    data = b"INERT PLAN FIXTURE, NOT RUST SOURCE: " + name.encode()
                    path.write_bytes(data)
                    pins[name] = {"sha256": hashlib.sha256(data).hexdigest(),
                                  "bytes": len(data), "mode": stat.S_IMODE(path.stat().st_mode)}
                if p01_state == "partial":
                    (root / P01_SOURCE_FILES[-1]).unlink()
                elif p01_state == "stale":
                    (root / P01_SOURCE_FILES[-1]).write_bytes(b"changed plan fixture")
            args = SimpleNamespace(profile="integrated", cargo_executable=None, offline=True,
                                   native_schema=False, run_dir=root / ".local/fixture-run")
            # Model the reviewed source version with inert, bounded files. The
            # real catalog still verifies their bytes/modes; no Rust runs here.
            # This also keeps a Darwin plan independent of Windows checkout EOL/modes.
            with patch.object(verify.p01_ci, "SOURCE_PINS", pins):
                steps = {s["name"]: s for s in verify.build_steps(root, args, "darwin", "arm64")}
        return root, steps

    def assert_macos_profile(self, root, steps, with_p01):
        rust = steps["rust-workspace"]
        self.assertIn("--include-ignored", rust["argv"])
        self.assertIn("--test-threads=1", rust["argv"])
        self.assertFalse(any(arg.startswith("--skip=") for arg in rust["argv"]))
        if with_p01:
            positions = [i for i, arg in enumerate(rust["argv"]) if arg == "--skip"]
            self.assertTrue(all(i + 1 < len(rust["argv"]) for i in positions))
            self.assertEqual([rust["argv"][i + 1] for i in positions], P01_NORMAL_EXCLUSIONS)
            self.assertTrue(set(IDS).isdisjoint(P01_NORMAL_EXCLUSIONS))
            self.assertIn("--message-format=json", rust["argv"])
            self.assertEqual(rust.get("summary_format"), "rust")
            self.assertTrue(rust.get("required", True))
            self.assertEqual(rust.get("p01_cohort"),
                             {"proof": ".local/fixture-run/p01-parent-cohorts.json"})
            self.assertIn("p01_cohort_proof", rust["required_artifact_kinds"])
            self.assertIn("p01_rust_build_log", rust["required_artifact_kinds"])
            self.assertEqual([a["path"] for a in rust["artifact_paths"]
                              if a["kind"] == "p01_cohort_proof"],
                             [".local/fixture-run/p01-parent-cohorts.json"])
            pdf = steps["p18-compiler-pdfs"]
            log = Path(pdf["argv"][pdf["argv"].index("--rust-log") + 1])
            self.assertEqual(log.parent, root / ".local/fixture-run")
            self.assertRegex(log.name, r"^\d+-rust-workspace[.]log$")
            self.assertEqual([a["path"] for a in rust["artifact_paths"]
                              if a["kind"] == "p01_rust_build_log"],
                             [log.relative_to(root).as_posix()])
        else:
            self.assertNotIn("--skip", rust["argv"])
            self.assertNotIn("p01_cohort", rust)
            self.assertNotIn("p01_cohort_proof", rust["required_artifact_kinds"])
            self.assertNotIn("p01_rust_build_log", rust["required_artifact_kinds"])
        self.assertFalse(rust.get("allowed_skips"))
        self.assertIn("p18_os_probe", rust["required_artifact_kinds"])
        self.assertIn("rust-workspace", steps["p18-compiler-pdfs"]["consumes_artifacts"])
        self.assertIn("p18-compiler-pdfs", steps["runtime-build"]["depends_on"])
        self.assertIn("rust-workspace", steps["p18-compiler-pdfs"]["depends_on"])
        self.assertTrue(steps["p18-compiler-pdfs"].get("required", True))
        self.assertIn("p18_pdf_validation", steps["p18-compiler-pdfs"]["required_artifact_kinds"])

    def test_macos_runs_all_ignored_cases_and_pdf_check_blocks_runtime(self):
        for state in ("absent", "valid"):
            with self.subTest(p01_source=state):
                root, steps = self.macos_profile(state)
                self.assert_macos_profile(root, steps, state == "valid")

    def test_p01_normal_exclusions_reject_extra_missing_renamed_or_foreign_skips(self):
        root, steps = self.macos_profile("valid")
        self.assert_macos_profile(root, steps, True)
        for change in ("fifth", "P18", "missing", "renamed", "alternate_spelling"):
            with self.subTest(change=change):
                altered = copy.deepcopy(steps)
                argv = altered["rust-workspace"]["argv"]
                if change == "fifth":
                    argv.extend(["--skip", "unknown_contract"])
                elif change == "P18":
                    argv.extend(["--skip", IDS[0]])
                elif change == "missing":
                    at = argv.index("--skip")
                    del argv[at:at + 2]
                elif change == "renamed":
                    argv[argv.index("--skip") + 1] += "_renamed"
                else:
                    argv.append("--skip=unknown_contract")
                with self.assertRaises(AssertionError):
                    self.assert_macos_profile(root, altered, True)

    def test_p01_proof_artifacts_and_P18_dependencies_cannot_be_dropped_or_optional(self):
        root, steps = self.macos_profile("valid")
        self.assert_macos_profile(root, steps, True)
        changes = {
            "missing-proof": lambda p: p["rust-workspace"].pop("p01_cohort"),
            "loose-proof": lambda p: p["rust-workspace"]["p01_cohort"].update(required=False),
            "proof-not-required": lambda p: p["rust-workspace"]["required_artifact_kinds"].remove("p01_cohort_proof"),
            "log-not-required": lambda p: p["rust-workspace"]["required_artifact_kinds"].remove("p01_rust_build_log"),
            "proof-not-captured": lambda p: p["rust-workspace"].update(artifact_paths=[a for a in p["rust-workspace"]["artifact_paths"] if a["kind"] != "p01_cohort_proof"]),
            "log-not-captured": lambda p: p["rust-workspace"].update(artifact_paths=[a for a in p["rust-workspace"]["artifact_paths"] if a["kind"] != "p01_rust_build_log"]),
            "optional-workspace": lambda p: p["rust-workspace"].update(required=False),
            "missing-runtime-PDF-dependency": lambda p: p["runtime-build"]["depends_on"].remove("p18-compiler-pdfs"),
            "missing-PDF-Rust-dependency": lambda p: p["p18-compiler-pdfs"]["depends_on"].remove("rust-workspace"),
            "missing-PDF-Rust-artifact": lambda p: p["p18-compiler-pdfs"]["consumes_artifacts"].remove("rust-workspace"),
            "optional-PDF": lambda p: p["p18-compiler-pdfs"].update(required=False),
        }
        for name, mutate in changes.items():
            with self.subTest(change=name):
                altered = copy.deepcopy(steps)
                mutate(altered)
                with self.assertRaises(AssertionError):
                    self.assert_macos_profile(root, altered, True)

    def test_p01_source_must_be_complete_and_match_its_pins_before_plan_qualification(self):
        root, steps = self.macos_profile("valid")
        self.assert_macos_profile(root, steps, True)
        for state in ("partial", "stale"):
            with self.subTest(p01_source=state), self.assertRaises((OSError, ValueError)):
                self.macos_profile(state)

    def test_pdf_content_requires_one_page_hungarian_text_and_resolved_reference(self):
        gate = self.gate()
        good = {"pages": 1, "text": "P18 Phase2 Árvíztűrő tükörfúrógép Második fájl A képlet száma: (1)."}
        gate.validate_pdf_record(good, 2)
        for data in ({**good, "pages": 0}, {**good, "pages": 2},
                     {**good, "text": good["text"].replace("(1)", "(??)")},
                     {**good, "text": "wrong document"}, {**good, "pages": True}):
            with self.subTest(data=data), self.assertRaises(ValueError):
                gate.validate_pdf_record(data, 2)
        with self.assertRaises(ValueError): gate.validate_pdf_record(good, 3)

    def test_render_comparison_rejects_missing_empty_or_different_outputs(self):
        gate = self.gate()
        with tempfile.TemporaryDirectory() as d:
            a, b = Path(d) / "a", Path(d) / "b"
            for first, second in ((b"", b""), (b"a", b"b"), (b"a", None)):
                a.write_bytes(first)
                if b.exists(): b.unlink()
                if second is not None: b.write_bytes(second)
                with self.subTest(second=second), self.assertRaises((ValueError, OSError)):
                    gate.validate_render_pair(a, b)
            a.write_bytes(b"render bytes"); b.write_bytes(b"render bytes")
            gate.validate_render_pair(a, b)

    def test_installer_payload_cannot_be_used_without_matching_captured_checksum(self):
        path = ROOT / "tools/texlive/prepare_ci.py"
        self.assertTrue(path.is_file(), "Verified dated CI provisioning is missing")
        provision = load("p18_ci_provision_contract", path)
        with tempfile.TemporaryDirectory() as d:
            payload, checksum = Path(d) / "install-tl-unx.tar.gz", Path(d) / "checksum"
            payload.write_bytes(b"tampered payload")
            checksum.write_text("0" * 128 + "  install-tl-unx.tar.gz\n")
            with self.assertRaises(ValueError): provision.verify_installer(payload, checksum)
            checksum.write_text("0" * 128 + "  another-file\n")
            with self.assertRaises(ValueError): provision.verify_installer(payload, checksum)

    def test_toolchain_replacement_cannot_reuse_provisioned_evidence(self):
        gate = self.gate()
        with tempfile.TemporaryDirectory() as d:
            prefix = Path(d).resolve() / "texlive"; prefix.mkdir()
            tool = prefix / "tool"; tool.write_bytes(b"verified original")
            receipt = {"schema": 1, "prefix": str(prefix), "toolchain": gate.toolchain_state(prefix)}
            (prefix.parent / "provision.json").write_text(json.dumps(receipt))
            gate.validate_toolchain(prefix)
            tool.write_bytes(b"changed executable")
            with self.assertRaises(ValueError): gate.validate_toolchain(prefix)

    def test_toolchain_mode_change_cannot_reuse_provisioned_evidence(self):
        gate = self.gate()
        with tempfile.TemporaryDirectory() as d:
            prefix = Path(d).resolve() / "texlive"; prefix.mkdir()
            tool = prefix / "tool"; tool.write_bytes(b"verified original")
            receipt = {"schema": 1, "prefix": str(prefix), "toolchain": gate.toolchain_state(prefix)}
            (prefix.parent / "provision.json").write_text(json.dumps(receipt))
            gate.validate_toolchain(prefix)
            tool.chmod(0o444)
            try:
                with self.assertRaises(ValueError): gate.validate_toolchain(prefix)
            finally: tool.chmod(0o644)

    def test_other_cargo_targets_cannot_supply_required_p18_case_identities(self):
        gate = self.gate()
        foreign = self.log().replace("tests/security_policy_contract.rs", "tests/another_contract.rs")
        with self.assertRaises(ValueError): gate.validate_workspace_log(foreign)

    def test_duplicate_short_names_and_native_diagnostics_in_other_targets_do_not_count(self):
        gate = self.gate()
        foreign = "     Running tests/another_contract.rs (target/debug/deps/another_contract-456def)\ntest review_focus_r2 ... ok\ntest macos::review_focus_r1 ... CoreGraphics diagnostic\nok\n"
        self.assertEqual(gate.validate_workspace_log(foreign + self.log() + foreign), IDS)
