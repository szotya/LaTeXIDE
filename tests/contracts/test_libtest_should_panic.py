"""Standard Rust should-panic output keeps the strict named-outcome contract."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("p12_libtest_parser", ROOT / "integration/registry/p01_ci.py")
P01 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P01)
EVIDENCE_SPEC = importlib.util.spec_from_file_location("p12_libtest_evidence", ROOT / "integration/registry/evidence.py")
EVIDENCE = importlib.util.module_from_spec(EVIDENCE_SPEC)
EVIDENCE_SPEC.loader.exec_module(EVIDENCE)


def output(lines, passed=1, failed=0, ignored=0, running=1):
    state = "FAILED" if failed else "ok"
    return (f"running {running} tests\n" + lines + "\n"
            f"test result: {state}. {passed} passed; {failed} failed; {ignored} ignored; "
            "0 measured; 0 filtered out; finished in 0.00s\n")


class LibtestShouldPanicContract(unittest.TestCase):
    def accepted(self, raw):
        try:
            return P01.libtest_observation(raw)
        except ValueError as error:
            self.fail(f"Standard Rust should-panic output was refused: {error}")

    def test_standard_qualifier_preserves_canonical_method_ids(self):
        raw = output("test normal ... ok\ntest nested::guard - should panic ... ok", passed=2, running=2)
        observed = self.accepted(raw)
        self.assertEqual(observed["events"], [("normal", "ok"), ("nested::guard", "ok")])
        self.assertEqual(observed["partitions"][0]["passed"], 2)

    def test_split_terminal_stays_owned_by_the_should_panic_method(self):
        raw = output("test guard - should panic ... \nthread 'guard' panicked at fixture.rs:1:1:\nTARGET_MARKER\nok")
        self.assertEqual(self.accepted(raw)["events"], [("guard", "ok")])

    def test_failed_should_panic_is_counted_as_failure(self):
        observed = self.accepted(output("test guard - should panic ... FAILED", passed=0, failed=1))
        self.assertEqual(observed["events"], [("guard", "FAILED")])
        self.assertEqual(observed["partitions"][0]["status"], "FAILED")
        self.assertEqual(observed["partitions"][0]["failed"], 1)

    def test_ignored_should_panic_is_never_promoted_to_pass(self):
        observed = self.accepted(output("test guard - should panic ... ignored", passed=0, ignored=1))
        self.assertEqual(observed["events"], [("guard", "ignored")])
        self.assertEqual(observed["partitions"][0]["passed"], 0)
        self.assertEqual(observed["partitions"][0]["ignored"], 1)

    def test_unknown_qualifiers_and_invalid_names_are_rejected(self):
        for line in ("test guard - should panic-ish ... ok", "test guard - should panic - should panic ... ok",
                     "test guard - unknown ... ok", "test 7guard - should panic ... ok"):
            with self.subTest(line=line), self.assertRaises(ValueError):
                P01.libtest_observation(output(line))

    def test_missing_and_unowned_terminal_outcomes_are_rejected(self):
        for lines in ("test guard - should panic ...", "test guard - should panic ... unknown",
                      "ok", "test guard - should panic ... ok\nok"):
            with self.subTest(lines=lines), self.assertRaises(ValueError):
                P01.libtest_observation(output(lines))

    def test_counts_headers_and_duplicate_method_guards_are_retained(self):
        invalid = [output("test guard - should panic ... ok", passed=2),
                   output("test guard - should panic ... ok", running=2),
                   output("test guard - should panic ... ok\ntest guard ... ok", passed=2, running=2),
                   output("test guard - should panic ... FAILED")]
        for raw in invalid:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                P01.libtest_observation(raw)

    def test_zero_named_methods_still_require_an_explicit_empty_role(self):
        raw = output("", passed=0, running=0)
        with self.assertRaises(ValueError):
            P01.libtest_observation(raw)
        self.assertEqual(P01.libtest_observation(raw, allow_empty=True)["events"], [])

    def test_private_child_qualifiers_cannot_bypass_required_evidence_rejection(self):
        with tempfile.TemporaryDirectory(prefix="libtest-child-") as directory:
            log = Path(directory) / "raw.log"
            for child in ("durable_contract::durable_crash_child", "runtime_contract::runtime_crash_child"):
                for qualifier in ("", " - should panic"):
                    with self.subTest(child=child, qualifier=qualifier):
                        log.write_text(output(f"test {child}{qualifier} ... ok"))
                        with self.assertRaises(ValueError):
                            EVIDENCE.rust_summary(log, "required")

    def test_failed_or_ignored_guards_do_not_satisfy_required_evidence(self):
        with tempfile.TemporaryDirectory(prefix="libtest-outcome-") as directory:
            log = Path(directory) / "raw.log"
            for raw in (output("test guard - should panic ... FAILED", passed=0, failed=1),
                        output("test guard - should panic ... ignored", passed=0, ignored=1)):
                with self.subTest(raw=raw):
                    log.write_text(raw)
                    with self.assertRaises(ValueError):
                        EVIDENCE.rust_summary(log, "required")


if __name__ == "__main__":
    unittest.main()
