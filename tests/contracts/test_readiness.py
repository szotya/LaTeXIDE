"""Source presence, P00 work and external acceptance remain separate facts."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReadinessContract(unittest.TestCase):
    def module(self):
        spec = importlib.util.spec_from_file_location("p00_readiness_test", ROOT / "integration/registry/readiness.py")
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module

    def fixture(self, root):
        for name in ["docs/ide-audit-2026-10-31/backlog.json", "docs/development/package-specs.json",
                     "integration/registry/owners.v1.json", "integration/registry/ports.v1.json", "integration/registry/p00-acceptance.v1.json"]:
            path = root / name; path.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(ROOT / name, path)
        (root / "integration/requests").mkdir()

    def report(self):
        names = ["generated-contracts", "development-plans", "coordinator-contracts", "documentation", "rust-workspace",
                 "isolated-module-seam", "integration-queue", "runtime-build", "ipc-roundtrip", "mcp-roundtrip", "native-build", "native-controls"]
        return {"status": "passed", "profile": "integrated", "source_sha256": "a" * 64,
                "checks": [{"name": n, "status": "passed"} for n in names]}

    def test_complete_canonical_inventory_preserves_baseline(self):
        data = self.module().capability_inventory(ROOT)
        self.assertEqual(len(data["requirements"]), 226)
        self.assertEqual(len(data["packages"]), 44)
        self.assertEqual(data["requirements"]["LANG-06"]["baseline"], "partial")
        self.assertEqual(data["requirements"]["QA-14"]["acceptance"], "unqualified")
        self.assertEqual(data["packages"]["P00"]["service_source"]["state"], "source_only")

    def test_service_or_test_file_never_becomes_provider_acceptance(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.fixture(root); module = self.module()
            before = module.capability_inventory(root)["packages"]["P01"]
            self.assertEqual(before["service_source"]["state"], "absent")
            for name in [before["service_source"]["path"], before["test_source"]["path"]]:
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text("source exists", encoding="utf-8")
            (root / "integration/requests/P01.json").write_text("{}", encoding="utf-8")
            after = module.capability_inventory(root)["packages"]["P01"]
            self.assertEqual(after["service_source"]["state"], "source_only")
            self.assertEqual(after["test_source"]["state"], "source_only")
            self.assertEqual(after["acceptance"], "unqualified")
            self.assertEqual(module.capability_inventory(root)["requirements"]["LANG-06"]["baseline"], "partial")

    def test_p00_independent_checks_can_pass_while_acceptance_waits_for_p03(self):
        data = self.module().p00_readiness(ROOT, self.report(), {"status": "current"})
        self.assertEqual(data["independent_status"], "passed")
        self.assertEqual(data["acceptance"], "partial")
        self.assertEqual({(b["requirement_id"], b["package_id"]) for b in data["external_package_blockers"]}, {("MCP-03", "P03")})
        self.assertEqual(set(data["requirements"]), {"QA-08", "QA-14", "OS-10"})
        self.assertEqual(data["requirements"]["OS-10"]["direct_prerequisites"], ["QA-14"])

    def test_provider_manifest_presence_never_closes_the_hard_dependency(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.fixture(root)
            (root / "integration/requests/P03.json").write_text("{}", encoding="utf-8")
            data = self.module().p00_readiness(root, self.report(), {"status": "current"})
            self.assertEqual(data["external_package_blockers"][0]["acceptance"], "unqualified")
            self.assertTrue(data["external_package_blockers"][0]["manifest_present"])
            self.assertEqual(data["acceptance"], "partial")

    def test_structure_shared_and_stale_evidence_do_not_claim_native_execution(self):
        module = self.module()
        for profile in ["structure", "shared"]:
            report = self.report(); report["profile"] = profile
            report["checks"] = [r for r in report["checks"] if r["name"] not in {"runtime-build", "ipc-roundtrip", "mcp-roundtrip", "native-build", "native-controls"}]
            data = module.p00_readiness(ROOT, report, {"status": "current"})
            native = next(c for c in data["requirements"]["QA-08"]["criteria"] if c["id"] == "mac-adapter-regression")
            self.assertEqual(native["status"], "unrun")
            self.assertNotEqual(data["independent_status"], "passed")
        for status in ["stale_source", "stale_artifact", "failed_report"]:
            data = module.p00_readiness(ROOT, self.report(), {"status": status})
            self.assertFalse(any(c["status"] == "passed" for r in data["requirements"].values() for c in r["criteria"]))

    def test_native_future_and_separate_program_scopes_are_not_invented_dag_edges(self):
        data = self.module().p00_readiness(ROOT, self.report(), {"status": "current"})
        scopes = {s["id"]: s for s in data["validation_scopes"]}
        self.assertEqual(scopes["windows11-native"]["owners"], ["P21"])
        self.assertEqual(scopes["linux-desktop"]["classification"], "future_platform")
        self.assertFalse(scopes["linux-desktop"]["blocks_current_mac_common_slice"])
        self.assertTrue(all(s["status"] == "unrun" for s in scopes.values()))
        self.assertEqual(data["requirements"]["QA-08"]["direct_prerequisites"], ["MCP-03"])

    def test_sidecar_cannot_remove_a_real_prerequisite_or_change_an_owner(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.fixture(root); path = root / "integration/registry/p00-acceptance.v1.json"
            data = json.loads(path.read_text()); data["requirements"]["QA-08"]["prerequisites"] = []
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError): self.module().p00_readiness(root, self.report(), {"status": "current"})
            owners = root / "integration/registry/owners.v1.json"; data = json.loads(owners.read_text())
            next(r for r in data["requirements"] if r["requirement_id"] == "QA-08")["package_id"] = "P01"
            owners.write_text(json.dumps(data))
            with self.assertRaises(ValueError): self.module().capability_inventory(root)

    def test_ownership_cycle_and_source_escape_are_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.fixture(root); path = root / "integration/registry/owners.v1.json"; data = json.loads(path.read_text())
            next(r for r in data["requirements"] if r["requirement_id"] == "MCP-03")["prerequisites"] = ["OS-10"]
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError): self.module().p00_readiness(root, self.report(), {"status": "current"})
            self.fixture_reset_owner(root)
            path = root / "integration/registry/ports.v1.json"; data = json.loads(path.read_text()); data["packages"][0]["service_entry"] = "../outside"
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError): self.module().capability_inventory(root)

    def test_empty_or_duplicate_independent_conditions_are_invalid(self):
        for mode in ["empty", "duplicate", "empty-checks"]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as d:
                root = Path(d); self.fixture(root); path = root / "integration/registry/p00-acceptance.v1.json"
                data = json.loads(path.read_text()); criteria = data["requirements"]["QA-08"]["criteria"]
                if mode == "empty": criteria.clear()
                elif mode == "duplicate": criteria.append(criteria[0].copy())
                else: criteria[0]["checks"] = []
                path.write_text(json.dumps(data))
                with self.assertRaises(ValueError): self.module().p00_readiness(root, self.report(), {"status": "current"})

    def fixture_reset_owner(self, root):
        shutil.copyfile(ROOT / "integration/registry/owners.v1.json", root / "integration/registry/owners.v1.json")
