"""Narrow regression tests with a pinned synthetic diagnostic template.

Default unittest discovery selects normal test methods. The two guard
functions are explicit fault-injection controls for the finite review harness.
Diagnostic content uses a pinned synthetic template and a host-native caller
locator. The historical raw capture stays in private evidence. No compiler or
consumer command is executed by this file.
"""
import base64
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

_HELPER_PATH = Path(__file__).resolve().parents[2] / "tools/verification/proc_macro_metadata_fixture.py"
_SPEC = importlib.util.spec_from_file_location("latexide_proc_macro_metadata_fixture_candidate", _HELPER_PATH)
fixtures = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = fixtures
HELPER_COMPILED_CODE = compile(_HELPER_PATH.read_bytes(), str(_HELPER_PATH), "exec", dont_inherit=True, optimize=0)
exec(HELPER_COMPILED_CODE, fixtures.__dict__)

POSITIVE_BASENAME = "libproc_macro-977a81cadb47f16e.rmeta"
EXPECTED_METADATA_PATH = (Path(__file__).resolve().parent / "fixtures" / "proc_macro_metadata" / "bad" / POSITIVE_BASENAME)
MALFORMED_BYTES = base64.b64decode("T1dOX0lOVkFMSURfUlVTVF9NRVRBREFUQV9GSUxFCg==")
STDERR_TEMPLATE = base64.b64decode("eyIkbWVzc2FnZV90eXBlIjoiZGlhZ25vc3RpYyIsIm1lc3NhZ2UiOiJmb3VuZCBpbnZhbGlkIG1ldGFkYXRhIGZpbGVzIGZvciBjcmF0ZSBgcHJvY19tYWNyb2AiLCJjb2RlIjp7ImNvZGUiOiJFMDc4NiIsImV4cGxhbmF0aW9uIjoiQSBtZXRhZGF0YSBmaWxlIHdhcyBpbnZhbGlkLlxuXG5FcnJvbmVvdXMgY29kZSBleGFtcGxlOlxuXG5gYGBpZ25vcmUgKG5lZWRzIGV4dGVybiBmaWxlcylcbnVzZSA6OmZvbzsgLy8gZXJyb3I6IGZvdW5kIGludmFsaWQgbWV0YWRhdGEgZmlsZXMgZm9yIGNyYXRlIGBmb29gXG5gYGBcblxuV2hlbiBsb2FkaW5nIGNyYXRlcywgZWFjaCBjcmF0ZSBtdXN0IGhhdmUgYSB2YWxpZCBtZXRhZGF0YSBmaWxlLlxuSW52YWxpZCBmaWxlcyBjb3VsZCBiZSBjYXVzZWQgYnkgZmlsZXN5c3RlbSBjb3JydXB0aW9uLFxuYW4gSU8gZXJyb3Igd2hpbGUgcmVhZGluZyB0aGUgZmlsZSwgb3IgKHJhcmVseSkgYSBidWcgaW4gdGhlIGNvbXBpbGVyIGl0c2VsZi5cblxuQ29uc2lkZXIgZGVsZXRpbmcgdGhlIGZpbGUgYW5kIHJlY3JlYXRpbmcgaXQsXG5vciByZXBvcnRpbmcgYSBidWcgYWdhaW5zdCB0aGUgY29tcGlsZXIuXG4ifSwibGV2ZWwiOiJlcnJvciIsInNwYW5zIjpbeyJmaWxlX25hbWUiOiIvc3ludGhldGljL3Byb2MtbWFjcm8tZml4dHVyZS9TMDIucnMiLCJieXRlX3N0YXJ0IjowLCJieXRlX2VuZCI6MjQsImxpbmVfc3RhcnQiOjEsImxpbmVfZW5kIjoxLCJjb2x1bW5fc3RhcnQiOjEsImNvbHVtbl9lbmQiOjI1LCJpc19wcmltYXJ5Ijp0cnVlLCJ0ZXh0IjpbeyJ0ZXh0IjoiZXh0ZXJuIGNyYXRlIHByb2NfbWFjcm87IiwiaGlnaGxpZ2h0X3N0YXJ0IjoxLCJoaWdobGlnaHRfZW5kIjoyNX1dLCJsYWJlbCI6bnVsbCwic3VnZ2VzdGVkX3JlcGxhY2VtZW50IjpudWxsLCJzdWdnZXN0aW9uX2FwcGxpY2FiaWxpdHkiOm51bGwsImV4cGFuc2lvbiI6bnVsbH1dLCJjaGlsZHJlbiI6W3sibWVzc2FnZSI6ImNvcnJ1cHQgbWV0YWRhdGEgZW5jb3VudGVyZWQgaW4gQEVYUEVDVEVEX01FVEFEQVRBX1BBVEhAIiwiY29kZSI6bnVsbCwibGV2ZWwiOiJub3RlIiwic3BhbnMiOltdLCJjaGlsZHJlbiI6W10sInJlbmRlcmVkIjpudWxsfV0sInJlbmRlcmVkIjoiZXJyb3JbRTA3ODZdOiBmb3VuZCBpbnZhbGlkIG1ldGFkYXRhIGZpbGVzIGZvciBjcmF0ZSBgcHJvY19tYWNyb2BcbiAtLT4gL3N5bnRoZXRpYy9wcm9jLW1hY3JvLWZpeHR1cmUvUzAyLnJzOjE6MVxuICB8XG4xIHwgZXh0ZXJuIGNyYXRlIHByb2NfbWFjcm87XG4gIHwgXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXG4gIHxcbiAgPSBub3RlOiBjb3JydXB0IG1ldGFkYXRhIGVuY291bnRlcmVkIGluIEBFWFBFQ1RFRF9NRVRBREFUQV9QQVRIQFxuXG4ifQp7IiRtZXNzYWdlX3R5cGUiOiJkaWFnbm9zdGljIiwibWVzc2FnZSI6ImFib3J0aW5nIGR1ZSB0byAxIHByZXZpb3VzIGVycm9yIiwiY29kZSI6bnVsbCwibGV2ZWwiOiJlcnJvciIsInNwYW5zIjpbXSwiY2hpbGRyZW4iOltdLCJyZW5kZXJlZCI6ImVycm9yOiBhYm9ydGluZyBkdWUgdG8gMSBwcmV2aW91cyBlcnJvclxuXG4ifQp7IiRtZXNzYWdlX3R5cGUiOiJkaWFnbm9zdGljIiwibWVzc2FnZSI6IkZvciBtb3JlIGluZm9ybWF0aW9uIGFib3V0IHRoaXMgZXJyb3IsIHRyeSBgcnVzdGMgLS1leHBsYWluIEUwNzg2YC4iLCJjb2RlIjpudWxsLCJsZXZlbCI6ImZhaWx1cmUtbm90ZSIsInNwYW5zIjpbXSwiY2hpbGRyZW4iOltdLCJyZW5kZXJlZCI6IkZvciBtb3JlIGluZm9ybWF0aW9uIGFib3V0IHRoaXMgZXJyb3IsIHRyeSBgcnVzdGMgLS1leHBsYWluIEUwNzg2YC5cbiJ9Cg==")
STDERR_TEMPLATE_SHA256 = "dc85c1492a1751df500a8e922b4d56313d61e544d97dd41b3f83d08f2eab2bc4"
SYNTHETIC_STDERR = STDERR_TEMPLATE.replace(
    b"@EXPECTED_METADATA_PATH@", json.dumps(str(EXPECTED_METADATA_PATH))[1:-1].encode("ascii"),
)
FILENAME_GUARD_MARKER = "P09_RMETA_FILENAME_GUARD_REQUIRED"
DIAGNOSTIC_GUARD_MARKER = "P09_RMETA_DIAGNOSTIC_ORACLE_REQUIRED"


def _write(path, payload, mode=0o600):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(fd, "wb") as stream:
        stream.write(payload)


def _rows(payload=SYNTHETIC_STDERR):
    return [json.loads(line) for line in payload.splitlines()]


def _encoded(rows):
    return ("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n").encode()


class ProcMacroMetadataFixtureTests(unittest.TestCase):
    def setUp(self):
        diagnostic_only = {
            "test_c02_exact_historical_diagnostic_synthetic_sample",
            "test_c11_nonrefusal_and_filename_diagnostics_rejected",
            "test_c12_parser_forgery_and_locator_rejected",
        }
        if not fixtures._FD_BACKEND and self._testMethodName not in diagnostic_only:
            self.skipTest("POSIX descriptor fixture backend unavailable")
        supplied = os.environ.get("P09_TEST_ROOT")
        if supplied:
            self.base = Path(supplied)
            self.base.mkdir(mode=0o700, parents=True, exist_ok=False)
        else:
            self.base = Path(tempfile.mkdtemp(prefix="proc-macro-rmeta-")).resolve()
        self.scope = self.base / "scope"
        self.scope.mkdir(mode=0o700)
        self.owner = self.new_owner(nonce=os.environ.get("P09_CASE_NONCE"))
        self.positive = self.base / "positive" / POSITIVE_BASENAME
        _write(self.positive, b"OWN_SYNTHETIC_POSITIVE_SENTINEL_NOT_RMETA", 0o400)
        self.positive_before = self.positive.read_bytes()

    def tearDown(self):
        # Preserve every synthetic source, output and refusal fixture.
        self.assertEqual(self.positive.read_bytes(), self.positive_before)

    def new_owner(self, payload=MALFORMED_BYTES, *, nonce=None, owner_mode=0o700, source_mode=0o600):
        owner = self.scope / (nonce or secrets.token_hex(16))
        owner.mkdir(mode=owner_mode)
        if payload is not None:
            _write(owner / "malformed.bytes", payload, source_mode)
        return owner

    def create(self, owner=None, basename=POSITIVE_BASENAME, source=None, scope=None):
        owner = self.owner if owner is None else owner
        return fixtures.create_malformed_rmeta(
            self.scope if scope is None else scope, owner, basename,
            owner / "malformed.bytes" if source is None else source,
        )

    def classify(self, payload=SYNTHETIC_STDERR, code=1, stdout=b"", expected=EXPECTED_METADATA_PATH):
        return fixtures.is_malformed_proc_macro_refusal(
            code, stdout, payload, expected_metadata_path=expected,
        )

    def test_c01_exact_owned_locator_and_positive_untouched(self):
        self.assertEqual(len(MALFORMED_BYTES), 31)
        self.assertEqual(hashlib.sha256(MALFORMED_BYTES).hexdigest(), fixtures.MALFORMED_SHA256)
        result = self.create()
        self.assertEqual(result.path, self.owner / "bad" / POSITIVE_BASENAME)
        self.assertEqual(result.path.read_bytes(), MALFORMED_BYTES)
        self.assertEqual(result.basename, POSITIVE_BASENAME)
        self.assertEqual(result.size, 31)
        self.assertEqual(result.sha256, fixtures.MALFORMED_SHA256)
        self.assertEqual(result.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(result.path.parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(self.positive.read_bytes(), self.positive_before)

    def test_c02_exact_historical_diagnostic_synthetic_sample(self):
        self.assertEqual(hashlib.sha256(STDERR_TEMPLATE).hexdigest(), STDERR_TEMPLATE_SHA256)
        self.assertTrue(self.classify())
        self.assertEqual(_rows()[0]["code"]["code"], "E0786")
        self.assertEqual(EXPECTED_METADATA_PATH.parent.name, "bad")
        self.assertEqual(EXPECTED_METADATA_PATH.name, POSITIVE_BASENAME)
        self.assertEqual(_rows()[0]["children"][0]["message"], "corrupt metadata encountered in " + str(EXPECTED_METADATA_PATH))

    def test_c03_invalid_basename_fail_closed(self):
        for name in [
            "bad.rmeta", "", "../" + POSITIVE_BASENAME, "/" + POSITIVE_BASENAME,
            "libstd-977a81cadb47f16e.rmeta", "libproc_macro-977A81CADB47F16E.rmeta",
            "libproc_macro-1.rmeta", "libproc_macro-977a81cadb47f16e.rlib",
            "libproc_macro-$(touch pwned).rmeta", "bad\x00.rmeta", None,
        ]:
            owner = self.new_owner()
            with self.assertRaises(fixtures.FixtureError):
                self.create(owner=owner, basename=name)
            self.assertFalse((owner / "bad").exists())

    def test_c04_missing_source_owner_or_scope_fail_closed(self):
        owner = self.new_owner(payload=None)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)
        self.assertFalse((owner / "bad").exists())
        missing = self.scope / secrets.token_hex(16)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=missing)
        missing_scope = self.base / "missing-scope"
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=missing_scope / secrets.token_hex(16), scope=missing_scope)

    def test_c05_size_and_hash_fail_closed(self):
        for payload in [b"", MALFORMED_BYTES[:-1], MALFORMED_BYTES + b"X", b"X" * 31]:
            owner = self.new_owner(payload)
            with self.assertRaises(fixtures.FixtureError):
                self.create(owner=owner)
            self.assertFalse((owner / "bad").exists())
            self.assertEqual((owner / "malformed.bytes").read_bytes(), payload)

    def test_c06_symlink_and_hardlink_refusal(self):
        owner = self.new_owner(payload=None)
        _write(owner / "other.bytes", MALFORMED_BYTES)
        os.symlink("other.bytes", owner / "malformed.bytes")
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)
        alias = self.scope / secrets.token_hex(16)
        os.symlink(self.owner.name, alias)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=alias)
        scope_alias = self.base / "scope-alias"
        os.symlink("scope", scope_alias)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=scope_alias / self.owner.name, scope=scope_alias)
        owner = self.new_owner()
        os.link(owner / "malformed.bytes", owner / "second-link.bytes")
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)

    def test_c07_wrong_scope_and_traversal_refusal(self):
        foreign = self.base / "other-scope"
        foreign.mkdir(mode=0o700)
        owner = foreign / secrets.token_hex(16)
        owner.mkdir(mode=0o700)
        _write(owner / "malformed.bytes", MALFORMED_BYTES)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)
        with self.assertRaises(fixtures.FixtureError):
            self.create(source=owner / "malformed.bytes")
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=Path("relative") / self.owner.name, scope=Path("relative"))
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=self.scope / ".." / self.scope.name / self.owner.name)
        with self.assertRaises(fixtures.FixtureError):
            self.create(scope=Path("/invalid\x00scope"))

    def test_c08_preexisting_destination_refusal(self):
        for kind in ["file", "directory", "symlink"]:
            owner = self.new_owner()
            bad = owner / "bad"
            if kind == "file":
                _write(bad, b"KEEP_EXISTING")
            elif kind == "directory":
                bad.mkdir(mode=0o700)
                _write(bad / POSITIVE_BASENAME, b"KEEP_EXISTING")
            else:
                os.symlink("../" + self.owner.name, bad)
            before = os.lstat(bad)
            with self.assertRaises(fixtures.FixtureError):
                self.create(owner=owner)
            after = os.lstat(bad)
            self.assertEqual((before.st_ino, before.st_mode, before.st_size), (after.st_ino, after.st_mode, after.st_size))
            if kind == "directory":
                self.assertEqual((bad / POSITIVE_BASENAME).read_bytes(), b"KEEP_EXISTING")

    def test_c09_atomic_file_collision_refusal(self):
        original = os.open
        injected = []
        def collide(path, flags, mode=0o777, *, dir_fd=None):
            if path == POSITIVE_BASENAME and flags & os.O_CREAT and not injected:
                fd = original(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=dir_fd)
                with os.fdopen(fd, "wb") as stream:
                    stream.write(b"KEEP_COLLISION")
                injected.append(True)
            return original(path, flags, mode, dir_fd=dir_fd)
        with mock.patch.object(fixtures.os, "open", side_effect=collide):
            with self.assertRaises(fixtures.FixtureError):
                self.create()
        self.assertEqual(injected, [True])
        self.assertEqual((self.owner / "bad" / POSITIVE_BASENAME).read_bytes(), b"KEEP_COLLISION")

    def test_c10_permission_owner_and_type_refusal(self):
        previous_umask = os.umask(0o077)
        self.addCleanup(os.umask, previous_umask)
        # A missing backend is an explicit platform skip, never a fixture PASS.
        with mock.patch.object(fixtures, "_FD_BACKEND", False):
            with self.assertRaisesRegex(fixtures.FixtureError, "POSIX descriptor backend unavailable"):
                self.create()
            unavailable = type(self)("test_c01_exact_owned_locator_and_positive_untouched")
            with self.assertRaisesRegex(unittest.SkipTest, "POSIX descriptor fixture backend unavailable"):
                unavailable.setUp()
            diagnostic = type(self)("test_c02_exact_historical_diagnostic_synthetic_sample")
            with mock.patch.object(Path, "mkdir", side_effect=RuntimeError("diagnostic fixture setup reached")):
                with self.assertRaisesRegex(RuntimeError, "diagnostic fixture setup reached"):
                    diagnostic.setUp()
        owner = self.new_owner(owner_mode=0o755)
        owner.chmod(0o755)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)
        owner = self.new_owner(source_mode=0o644)
        (owner / "malformed.bytes").chmod(0o644)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)
        owner = self.new_owner(payload=None)
        (owner / "malformed.bytes").mkdir(mode=0o700)
        with self.assertRaises(fixtures.FixtureError):
            self.create(owner=owner)
        actual = os.getuid()
        with mock.patch.object(fixtures.os, "getuid", return_value=actual + 1):
            with self.assertRaises(fixtures.FixtureError):
                self.create()

    def test_c11_nonrefusal_and_filename_diagnostics_rejected(self):
        for code in [0, 101, True, 1.0, None]:
            self.assertFalse(self.classify(code=code))
        self.assertFalse(self.classify(stdout=b"unexpected stdout"))
        rows = _rows()
        rows[0]["message"] = "extern location for proc_macro is of an unknown type: /synthetic/metadata/bad.rmeta"
        rows[0]["code"]["code"] = "E0463"
        self.assertFalse(self.classify(_encoded(rows)))
        rows = _rows()
        rows[0]["message"] = "found invalid metadata files for crate `another_crate`"
        self.assertFalse(self.classify(_encoded(rows)))

    def test_c12_parser_forgery_and_locator_rejected(self):
        self.assertFalse(self.classify(b"not JSON"))
        self.assertFalse(self.classify(b"\xff"))
        self.assertFalse(self.classify(SYNTHETIC_STDERR + b" " * fixtures.MAX_DIAGNOSTIC_BYTES))
        duplicate = SYNTHETIC_STDERR.replace(b'"level":"error"', b'"level":"error","level":"error"', 1)
        self.assertFalse(self.classify(duplicate))
        rows = _rows()
        rows[0]["code"]["explanation"] = float("nan")
        self.assertFalse(self.classify(_encoded(rows)))
        rows = _rows()
        rows[0]["command"] = "must never execute"
        self.assertFalse(self.classify(_encoded(rows)))
        rows = _rows()
        rows[0]["children"][0]["message"] = "corrupt metadata encountered in /other/bad/" + POSITIVE_BASENAME
        self.assertFalse(self.classify(_encoded(rows)))
        self.assertFalse(self.classify(expected=Path("/other/bad") / POSITIVE_BASENAME))
        self.assertFalse(self.classify(expected=Path("/other/bad/bad.rmeta")))
        self.assertFalse(self.classify(_encoded(_rows() + [_rows()[0]])))
        self.assertFalse(self.classify(expected=Path("/invalid\x00scope/bad") / POSITIVE_BASENAME))


def guard_filename_validation_removed(case):
    with mock.patch.object(fixtures, "_checked_basename", side_effect=lambda value: value):
        rejected = False
        try:
            case.create(basename="bad.rmeta")
        except fixtures.FixtureError:
            rejected = True
        assert rejected, FILENAME_GUARD_MARKER


def guard_diagnostic_oracle_removed(case):
    rows = _rows()
    rows[0]["message"] = "filename rejection is not metadata validation"
    rows[0]["code"]["code"] = "E0463"
    with mock.patch.object(fixtures, "_metadata_error_matches", return_value=True):
        assert not case.classify(_encoded(rows)), DIAGNOSTIC_GUARD_MARKER


class ProcMacroMetadataPortableContractTests(unittest.TestCase):
    def test_unsupported_backend_refuses_before_handling_paths(self):
        if not fixtures._FD_BACKEND:
            with self.assertRaisesRegex(fixtures.FixtureError, "^required POSIX descriptor backend unavailable$"):
                fixtures.create_malformed_rmeta(object(), object(), object(), object())
        with mock.patch.object(fixtures, "_FD_BACKEND", False), \
             mock.patch.object(fixtures, "_absolute", side_effect=AssertionError("unsupported backend handled a path")):
            with self.assertRaisesRegex(fixtures.FixtureError, "^required POSIX descriptor backend unavailable$"):
                fixtures.create_malformed_rmeta(object(), object(), object(), object())

    def test_ci_skip_scope_is_exact_windows_only(self):
        root = Path(__file__).resolve().parents[2]
        spec = importlib.util.spec_from_file_location("rmeta_portable_verify", root / "integration/registry/verify.py")
        verify = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(verify)
        methods = [
            "test_c01_exact_owned_locator_and_positive_untouched",
            "test_c03_invalid_basename_fail_closed",
            "test_c04_missing_source_owner_or_scope_fail_closed",
            "test_c05_size_and_hash_fail_closed",
            "test_c06_symlink_and_hardlink_refusal",
            "test_c07_wrong_scope_and_traversal_refusal",
            "test_c08_preexisting_destination_refusal",
            "test_c09_atomic_file_collision_refusal",
            "test_c10_permission_owner_and_type_refusal",
        ]
        expected = ["test_proc_macro_metadata_fixture.ProcMacroMetadataFixtureTests." + name for name in methods]
        args = SimpleNamespace(run_dir=root / ".local/rmeta-plan-fixture", cargo_executable=None,
                               offline=True, profile="structure", native_schema=False)
        summary = {"schema": 1, "suite": "coordinator", "tests": 10, "passed": 1, "skipped": 9,
                   "failures": 0, "errors": 0, "skipped_ids": expected}
        for platform, machine in [("win32", "AMD64"), ("linux", "x86_64"), ("darwin", "arm64")]:
            with self.subTest(platform=platform):
                steps = verify.build_steps(root, args, platform, machine)
                allowed = next(s for s in steps if s["name"] == "coordinator-contracts").get("allowed_skips", [])
                self.assertEqual(allowed, expected if platform == "win32" else [])
                if platform == "win32":
                    self.assertEqual(verify.validate_summary(summary, "coordinator", allowed)["skipped"], 9)
                else:
                    with self.assertRaisesRegex(ValueError, "Unqualified test skip"):
                        verify.validate_summary(summary, "coordinator", allowed)
                for forbidden in ["test_verification.ExistingRequiredCase.test_required",
                                  "test_proc_macro_metadata_fixture.ProcMacroMetadataFixtureTests.test_c02_exact_historical_diagnostic_synthetic_sample",
                                  "test_proc_macro_metadata_fixture.ProcMacroMetadataPortableContractTests.test_unsupported_backend_refuses_before_handling_paths"]:
                    invalid = {**summary, "tests": 2, "skipped": 1, "skipped_ids": [forbidden]}
                    with self.assertRaisesRegex(ValueError, "Unqualified test skip"):
                        verify.validate_summary(invalid, "coordinator", allowed)
                with self.assertRaises(ValueError):
                    verify.validate_summary({**summary, "tests": 11}, "coordinator", allowed)


if __name__ == "__main__":
    unittest.main()
