"""Derived P00 progress must follow the final actual evidence state."""
import json
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch
import test_evidence as fixtures

verify = fixtures.verify


class ReportReadinessIntegration(unittest.TestCase):
    def test_failed_coordinator_diagnostics_are_bounded_and_success_stays_quiet(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            row = verify.execute_step("coordinator-contracts", [sys.executable, "-c", "raise ValueError('synthetic coordinator cause')"], root, root / "failure.log", 5)
            details = verify.failure_diagnostics([row])
            self.assertIn("synthetic coordinator cause", details[0]["tail"])
            self.assertEqual(details[0]["name"], "coordinator-contracts")
            (root / "failure.log").write_bytes(b"x" * 50000)
            self.assertLessEqual(len(verify.failure_diagnostics([row])[0]["tail"].encode()), 8192)
            row["status"] = "passed"
            self.assertEqual(verify.failure_diagnostics([row]), [])

    def test_report_includes_fresh_inventory_and_explicit_external_dependency(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = fixtures.EvidenceContract.fixture(self, root)
            result = verify.finalize_report(root, report, steps)
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["evidence_validation"]["status"], "current")
            self.assertEqual(len(result["capability_inventory"]["requirements"]), 226)
            self.assertEqual(result["capability_inventory"]["requirements"]["LANG-06"]["baseline"], "partial")
            self.assertEqual(result["p00_readiness"]["external_package_blockers"][0]["requirement_id"], "MCP-03")
            self.assertEqual(result["p00_readiness"]["acceptance"], "partial")

    def test_late_source_change_invalidates_derived_readiness_too(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = fixtures.EvidenceContract.fixture(self, root)
            original = verify.readiness.capability_inventory
            def late_change(path):
                value = original(path); (root / "crates/late.rs").write_text("late source", encoding="utf-8"); return value
            with patch.object(verify.readiness, "capability_inventory", side_effect=late_change):
                result = verify.finalize_report(root, report, steps)
            self.assertEqual(result["status"], "source_changed")
            self.assertEqual(result["evidence_validation"]["status"], "stale_source")
            self.assertEqual(result["p00_readiness"]["evidence_status"], "stale_source")
            self.assertFalse(any(c["status"] == "passed" for r in result["p00_readiness"]["requirements"].values() for c in r["criteria"]))

    def test_cached_complete_claim_is_recomputed_after_artifact_change(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = fixtures.EvidenceContract.fixture(self, root)
            report["p00_readiness"] = {"acceptance": "passed", "independent_status": "passed"}
            (root / "palette").write_bytes(b"changed")
            result = verify.finalize_report(root, report, steps)
            self.assertEqual(result["status"], "failed")
            self.assertEqual(result["evidence_validation"]["status"], "stale_artifact")
            self.assertEqual(result["p00_readiness"]["acceptance"], "partial")
            self.assertNotEqual(result["p00_readiness"]["independent_status"], "passed")

    def test_artifact_changed_during_inventory_invalidates_derived_progress(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); steps, report = fixtures.EvidenceContract.fixture(self, root); original = verify.readiness.capability_inventory
            def late_change(path):
                value = original(path); (root / "palette").write_bytes(b"late resource"); return value
            with patch.object(verify.readiness, "capability_inventory", side_effect=late_change):
                result = verify.finalize_report(root, report, steps)
            self.assertEqual(result["status"], "failed")
            self.assertEqual(result["evidence_validation"]["status"], "stale_artifact")
            self.assertFalse(any(c["status"] == "passed" for r in result["p00_readiness"]["requirements"].values() for c in r["criteria"]))

    def test_source_set_changes_during_the_final_evidence_pass_invalidate_every_derived_state(self):
        for mode in ["add", "remove", "change"]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as d:
                root = Path(d); steps, report = fixtures.EvidenceContract.fixture(self, root)
                report["requirements"] = {"QA-14": {"current_verification": {"status": "passed", "checks": ["unit"]}}}
                original = verify.evidence.read_json; calls = 0
                def late_change(path, *args):
                    nonlocal calls
                    value = original(path, *args); calls += 1
                    if calls == 2:
                        if mode == "add": (root / "crates/new-late.rs").write_text("late source", encoding="utf-8")
                        elif mode == "remove": (root / "crates/source.rs").unlink()
                        else: (root / "crates/source.rs").write_text("late replacement", encoding="utf-8")
                    return value
                with patch.object(verify.evidence, "read_json", side_effect=late_change):
                    result = verify.finalize_report(root, report, steps)
                self.assertEqual(result["status"], "source_changed")
                self.assertEqual(result["evidence_validation"]["status"], "stale_source")
                self.assertEqual(result["evidence_validation"]["identities"]["source"], "stale_source")
                self.assertEqual(result["capability_inventory"]["freshness"], "stale_source")
                self.assertEqual(result["requirements"]["QA-14"]["current_verification"]["status"], "stale_source")
                self.assertEqual(result["p00_readiness"]["evidence_status"], "stale_source")
                self.assertFalse(any(c["status"] == "passed" for r in result["p00_readiness"]["requirements"].values() for c in r["criteria"]))
