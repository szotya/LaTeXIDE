"""Derived P00 progress must follow the final actual evidence state."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import test_evidence as fixtures

verify = fixtures.verify


class ReportReadinessIntegration(unittest.TestCase):
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
