"""P19 adapter tests. Synthetic helpers never qualify a product provider."""
import importlib.util
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
FAULT_RECORDS = []
MODEL_RECORDS = []


def adapter():
    path = HERE / "run.py"
    assert path.is_file(), "P19 adapter is missing; DTO fixtures are not a service"
    spec = importlib.util.spec_from_file_location("p19_owned_adapter", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class HarnessTests(unittest.TestCase):
    def api(self, name):
        value = getattr(adapter(), name, None)
        self.assertTrue(callable(value), "missing P19 behavior: " + name)
        return value

    def case(self, **changes):
        return {"schema": 1, "case_id": "own-after-sync", "profile": "multifile-hu",
                "point": "after_journal_sync", "acknowledged_revision": 7,
                "seed": 1901, "mode": "normal", "timeout_ms": 1000, **changes}

    def fault(self, **changes):
        return self.run_case(self.case(**changes))

    def run_case(self, case, executable=None):
        result = self.api("run_fault_case")(case, executable or HERE / "synthetic_helper.py")
        FAULT_RECORDS.append({"case": case, "state": result["state"], "reason": result["reason"],
                              "passed": result["passed"], "producer_counts": result["producer_counts"],
                              "binding": result.get("binding"), "processes": result["processes"],
                              "receipt_path": str(Path(result["run_directory"]) / "result.json") if result.get("run_directory") else None})
        return result

    def test_contract_v1_first_slice(self):
        outcome = adapter().expected_fault_outcome("after_journal_sync", 7)
        self.assertEqual(outcome, {"minimum_recovered_revision": 7,
                                   "disk_may_change": False,
                                   "partial_edit_allowed": False})

    def test_oracle_validates_wire_points_and_lossless_u64(self):
        oracle = adapter().expected_fault_outcome
        for point in ("before_journal_sync", "after_journal_sync", "before_rename", "after_rename", "mid_frame"):
            outcome = oracle(point, 9007199254740993)
            self.assertEqual(outcome["minimum_recovered_revision"], 9007199254740993)
            self.assertFalse(outcome["partial_edit_allowed"])
            self.assertEqual(outcome["disk_may_change"], point == "after_rename")
        for rev in (True, -1, 2**64, 7.0, "7"):
            with self.subTest(rev=rev), self.assertRaises(ValueError):
                oracle("after_journal_sync", rev)
        with self.assertRaises(ValueError):
            oracle("AfterJournalSync", 7)

    def test_v1_request_decoder_uses_request_shape_only(self):
        decode = self.api("decode_contract_request")
        fixture = json.loads((HERE.parents[1] / "tests/contracts/v1/p19.json").read_text(encoding="utf-8"))
        request = copy.deepcopy(fixture["cases"][0]["request"])
        request["payload"] = {"point": "after_journal_sync", "acknowledged_revision": 9007199254740993}
        self.assertEqual(decode(json.dumps(request).encode())["minimum_recovered_revision"], 9007199254740993)
        for mutate in (
            lambda r: r["version"].update(major=2),
            lambda r: r["payload"].update(unknown=True),
            lambda r: r["payload"].update(acknowledged_revision=True),
            lambda r: r.update(package_id="P01"),
        ):
            broken = copy.deepcopy(request); mutate(broken)
            with self.assertRaises(ValueError):
                decode(json.dumps(broken).encode())
        for raw in (b"{", b"{}", b"x" * 1048577, b'{"version":{"major":1,"minor":0},"version":null}'):
            with self.assertRaises(ValueError):
                decode(raw)

    def test_review_focus_r1_seeded_replay_and_minimal_repro(self):
        properties = self.api("run_properties")
        first = properties(seed=1901, max_cases=16, time_budget_ms=1000)
        second = properties(seed=1901, max_cases=16, time_budget_ms=1000)
        self.assertEqual(first, second)
        self.assertEqual((first["state"], first["actual_cases"]), ("pass", 16))
        self.assertEqual(first["rejections"], {"revision_conflict": 4, "invalid_utf16": 4, "quota": 4, "invalid_schema": 4})
        MODEL_RECORDS.append({"producer": "synthetic-property-model", "result": first, "replay_equal": first == second})
        shrink = self.api("minimize_reproducer")
        # The predicate runs the deliberately broken own model; the reducer
        # must isolate the operation that actually violates atomicity.
        check = self.api("negative_partial_edit_model")
        result = shrink(["read", "partial_edit", "read", "noop"], check, seed=1901)
        self.assertEqual(result["operations"], ["partial_edit"])
        self.assertEqual(result["seed"], 1901)
        self.assertTrue(check(result["operations"]))
        self.assertFalse(check(["read", "noop"]))
        MODEL_RECORDS.append({"producer": "own-negative-model-reducer", "result": result})

    def test_review_focus_r2_recovery_and_actual_disk_are_bound(self):
        result = self.fault()
        self.assertEqual(result["state"], "pass", result)
        self.assertEqual(result["scope"], "synthetic-helper-only")
        self.assertEqual(result["observed"]["recovered_revision"], 7)
        self.assertEqual(result["observed"]["disk_variant"], "before")
        self.assertEqual([e["seq"] for e in result["events"]], [1, 2])
        self.assertEqual([e["phase"] for e in result["events"]], ["pre_fault", "post_recovery"])
        self.assertEqual(result["processes"][0]["exit"], 86)
        self.assertEqual(result["processes"][1]["exit"], 0)
        for event in result["events"]:
            for key in ("nonce", "input_sha256", "source_sha256", "executable_sha256", "interpreter_sha256"):
                self.assertEqual(event[key], result["binding"][key])
        for artifact in result["artifacts"]:
            self.assertEqual(hashlib.sha256(Path(artifact["path"]).read_bytes()).hexdigest(), artifact["sha256"])
        self.assertTrue(self.api("recheck_fault_result")(result)["valid"])

    def test_review_focus_r3_corpus_mismatch_never_updates_baseline(self):
        load = self.api("load_profiles")
        profiles = load()
        self.assertEqual([p["id"] for p in profiles], ["singlefile", "multifile-hu"])
        for profile in profiles:
            self.assertEqual(profile["expected"]["compile_state"], "unrun")
            self.assertEqual(profile["platforms"], {"macos": "unrun", "windows11": "unrun", "linux": "unrun"})
            self.assertTrue(profile["recipe"]["argv"])
            self.assertTrue(profile["missing_data"])
        scratch = Path(os.environ["P19_SCRATCH"])
        with tempfile.TemporaryDirectory(dir=scratch / "tmp") as directory:
            copied = Path(directory) / "corpus"
            shutil.copytree(HERE / "corpus", copied)
            manifest = copied / "manifest.json"; before = manifest.read_bytes()
            (copied / "singlefile/main.tex").write_bytes(b"corrupted fixture")
            with self.assertRaisesRegex(ValueError, "fixture_identity"):
                load(manifest)
            self.assertEqual(manifest.read_bytes(), before)

    def test_review_focus_r4_direct_edit_explicit_save_and_undo_trace(self):
        session = self.api("run_model_session")
        original = b"\xef\xbb\xbf" + "\ufeffÁrvíz\r\n".encode()
        result = session(original, [
            {"op": "agent_edit", "expected_revision": 1, "start_utf16": 6, "length_utf16": 0, "replacement": "😀"},
            {"op": "agent_edit", "expected_revision": 1, "start_utf16": 0, "length_utf16": 0, "replacement": "stale"},
            {"op": "explicit_save", "expected_revision": 2},
            {"op": "undo", "expected_revision": 2},
        ])
        self.assertEqual([t["seq"] for t in result["trace"]], [1, 2, 3, 4])
        self.assertEqual([t["status"] for t in result["trace"]], ["ok", "revision_conflict", "ok", "ok"])
        self.assertEqual([t["revision"] for t in result["trace"]], [2, 2, 2, 3])
        self.assertEqual(result["text"], "\ufeffÁrvíz\n")
        self.assertEqual(bytes(result["disk"]), b"\xef\xbb\xbf" + "\ufeffÁrvíz😀\r\n".encode())
        self.assertTrue(result["dirty"])
        self.assertEqual(result["scope"], "synthetic-model-only")
        MODEL_RECORDS.append({"producer": "synthetic-edit-save-undo-model", "result": result})

    def test_review_focus_r5_timeout_is_interrupted_not_pass(self):
        result = self.fault(mode="timeout", timeout_ms=100)
        self.assertEqual(result["state"], "timeout", result)
        self.assertFalse(result["passed"])
        self.assertTrue(result["processes"][0]["timeout"])
        self.assertEqual(result["producer_counts"]["pass"], 0)

    def test_partial_edit_is_detected_from_bytes_even_when_helper_denies_it(self):
        result = self.fault(mode="partial_edit", point="after_rename")
        self.assertEqual((result["state"], result["reason"]), ("fail", "partial_disk_edit"), result)
        self.assertFalse(result["passed"])
        self.assertFalse(result["events"][0]["partial_edit_allowed"])
        replay = self.run_case(result["minimal_reproducer"])
        self.assertEqual(replay["reason"], "partial_disk_edit")
        self.assertEqual(replay["binding"]["input_sha256"], result["binding"]["input_sha256"])
        self.assertNotEqual(replay["binding"]["nonce"], result["binding"]["nonce"])

    def test_missing_zero_skip_and_malformed_producers_cannot_pass(self):
        for mode, state, reason in (
            ("zero_case", "fail", "zero_case"),
            ("skip", "skip", "producer_skip"),
            ("wrong_nonce", "fail", "event_binding"),
            ("bad_schema", "fail", "event_schema"),
            ("missing_post", "fail", "missing_event"),
            ("wrong_revision", "fail", "recovered_revision"),
            ("truncated_journal", "fail", "recovery_exit"),
            ("unexpected_exit", "fail", "fault_exit"),
        ):
            with self.subTest(mode=mode):
                result = self.fault(mode=mode)
                self.assertEqual((result["state"], result["reason"]), (state, reason), result)
                self.assertFalse(result["passed"])
        missing = self.run_case(self.case(), HERE / "missing-helper.py")
        self.assertEqual(missing["state"], "blocked")
        self.assertEqual(missing["producer_counts"]["actual_cases"], 0)

    def test_all_fault_points_preserve_ack_and_atomic_disk_in_own_helper(self):
        for point, variant in (("before_journal_sync", "before"), ("after_journal_sync", "before"),
                               ("before_rename", "before"), ("after_rename", "after"), ("mid_frame", "before")):
            with self.subTest(point=point):
                result = self.fault(point=point)
                self.assertEqual(result["state"], "pass", result)
                self.assertEqual(result["observed"]["recovered_revision"], 7)
                self.assertEqual(result["observed"]["disk_variant"], variant)

    def test_truncated_frame_and_journal_tail_control(self):
        decode = self.api("decode_frame")
        raw = b'{"ok":true}'
        self.assertEqual(decode(struct.pack(">I", len(raw)) + raw), {"ok": True})
        for frame in (b"", b"\0\0", struct.pack(">I", 11) + b"{", struct.pack(">I", 1048577)):
            with self.assertRaises(ValueError):
                decode(frame)
        result = self.fault(mode="journal_tail", point="mid_frame")
        self.assertEqual(result["state"], "pass", result)
        self.assertEqual(result["observed"]["recovered_revision"], 7)
        self.assertTrue(result["observed"]["truncated_tail_rejected"])

    def test_unicode_utf16_schema_revision_and_quota_nonmutation_controls(self):
        session = self.api("run_model_session")
        for operation, reason in (
            ({"op": "agent_edit", "expected_revision": 1, "start_utf16": 1, "length_utf16": 0, "replacement": "x"}, "invalid_utf16"),
            ({"op": "agent_edit", "expected_revision": 0, "start_utf16": 0, "length_utf16": 0, "replacement": "x"}, "revision_conflict"),
            ({"op": "agent_edit", "expected_revision": 1, "start_utf16": 0, "length_utf16": 0, "replacement": "x" * 1048576}, "quota"),
            ({"op": "agent_edit", "expected_revision": 1, "start_utf16": 0, "length_utf16": 0, "replacement": "x", "extra": True}, "invalid_schema"),
        ):
            with self.subTest(reason=reason):
                result = session("😀ár\n".encode(), [operation])
                self.assertEqual(result["trace"][0]["status"], reason)
                self.assertEqual(result["text"], "😀ár\n")
                self.assertEqual(result["revision"], 1)
                self.assertFalse(result["dirty"])

    def test_fresh_receipt_recheck_rejects_log_replacement(self):
        result = self.fault(profile="singlefile")
        self.assertEqual(result["state"], "pass", result)
        Path(result["artifacts"][0]["path"]).write_bytes(b"intentional own artifact corruption")
        check = self.api("recheck_fault_result")(result)
        self.assertEqual((check["valid"], check["reason"]), (False, "artifact_changed"))

    def test_case_schema_and_fixed_budgets_reject_empty_or_unbounded_work(self):
        suite = self.api("run_suite")
        for cases in ([], [self.case()] * 17):
            result = suite(cases, HERE / "synthetic_helper.py", max_cases=16, time_budget_ms=3000)
            self.assertEqual(result["state"], "fail")
            self.assertFalse(result["passed"])
        for changes in ({"schema": 2}, {"acknowledged_revision": True}, {"timeout_ms": 999999}, {"point": "unknown"}):
            result = self.fault(**changes)
            self.assertEqual(result["state"], "fail")
            self.assertEqual(result["reason"], "invalid_case")
        for count in (0, 17):
            with self.assertRaises(ValueError):
                self.api("run_properties")(seed=1901, max_cases=count, time_budget_ms=1000)

    def test_malformed_recovery_events_and_journal_records_cannot_pass(self):
        for mode, reason in (("bool_revision", "event_schema"), ("partial_recovered", "partial_recovered_edit"),
                             ("wrong_seq", "event_binding"), ("truncated_log", "event_schema"),
                             ("zero_claim", "event_schema"), ("bad_journal_schema", "recovery_exit"),
                             ("bad_journal_revision", "recovery_exit")):
            with self.subTest(mode=mode):
                result = self.fault(mode=mode, acknowledged_revision=1)
                self.assertEqual((result["state"], result["reason"]), ("fail", reason), result)
                self.assertFalse(result["passed"])

    def test_corpus_materializes_bom_crlf_and_logical_feff_independently(self):
        profiles = self.api("load_profiles")()
        profile = profiles[1]
        files = {item["path"]: bytes(item["raw_bytes"]) for item in profile["files"] if item["role"] in ("tex", "bib")}
        self.assertTrue(files["fejezet.tex"].startswith(b"\xef\xbb\xbf\xef\xbb\xbf"))
        for raw in files.values():
            self.assertTrue(raw.startswith(b"\xef\xbb\xbf"))
            self.assertGreater(raw.count(b"\r\n"), 0)
            self.assertNotIn(b"\n", raw.replace(b"\r\n", b""))

    def test_counting_distinguishes_subtest_failures_from_test_methods(self):
        summarize = globals().get("summarize_tests")
        self.assertTrue(callable(summarize), "missing honest test-method/subtest accounting")
        class TwoFailures(unittest.TestCase):
            def runTest(self):
                for index in range(2):
                    with self.subTest(index=index):
                        self.assertEqual(index, 99)
        result = unittest.TextTestRunner(stream=io.StringIO()).run(TwoFailures())
        counts = summarize(result)
        self.assertEqual((counts["actual_cases"], counts["pass"], counts["fail"], counts["error"]), (1, 0, 1, 0))
        self.assertEqual(counts["assertion_failures"], 2)

    def test_receipt_cannot_drop_an_artifact_or_change_its_verdict(self):
        result = self.fault(profile="singlefile")
        self.assertEqual(result["state"], "pass", result)
        missing = copy.deepcopy(result); missing["artifacts"].pop()
        check = self.api("recheck_fault_result")
        self.assertEqual(check(missing), {"valid": False, "reason": "artifact_changed"})
        altered = copy.deepcopy(result); altered["reason"] = "invented-result"
        self.assertEqual(check(altered), {"valid": False, "reason": "receipt_changed"})


class RecordingResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.selected_case_ids = []
        self.subtests = []

    def startTest(self, test):
        self.selected_case_ids.append(test.id())
        super().startTest(test)

    def addSubTest(self, test, subtest, error):
        state = "pass" if error is None else "fail" if issubclass(error[0], test.failureException) else "error"
        self.subtests.append({"case": subtest.id(), "state": state})
        super().addSubTest(test, subtest, error)


def summarize_tests(result, total=None):
    # A test method can have many failed subtests. They cannot be subtracted
    # from the method count: that would fabricate negative PASS counts.
    def parent(test):
        return getattr(test, "test_case", test).id()
    errors = {parent(test) for test, _ in result.errors}
    failures = {parent(test) for test, _ in result.failures} - errors
    skips = {parent(test) for test, _ in result.skipped} - errors - failures
    return {"producer": "python-unittest/P19", "actual_cases": result.testsRun,
              "pass": result.testsRun - len(failures | errors | skips),
              "fail": len(failures), "error": len(errors),
              "skip": len(skips), "ignored": 0, "filtered": max(0, (total or result.testsRun) - result.testsRun),
              "blocked": 0, "unrun": 0,
              "assertion_failures": len(result.failures),
              "selected_case_ids": getattr(result, "selected_case_ids", []),
              "subtests": getattr(result, "subtests", []),
              "failures": [test.id() for test, _ in result.failures],
              "errors": [test.id() for test, _ in result.errors],
              "skips": [{"case": test.id(), "reason": reason} for test, reason in result.skipped]}


if __name__ == "__main__":
    total = unittest.defaultTestLoader.loadTestsFromTestCase(HarnessTests).countTestCases()
    program = unittest.main(exit=False, testRunner=unittest.TextTestRunner(verbosity=2, resultclass=RecordingResult))
    result = program.result
    report = summarize_tests(result, total)
    report["fault_invocations"] = FAULT_RECORDS
    report["model_runs"] = MODEL_RECORDS
    report["scope"] = "P19 own adapter/helper/model self-check; no product provider invoked"
    if os.environ.get("P19_TEST_REPORT"):
        Path(os.environ["P19_TEST_REPORT"]).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    sys.exit(0 if result.testsRun > 0 and result.wasSuccessful() else 1)
