import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("acquire_tools", ROOT / "scripts/acquire_tools.py")
acquire = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(acquire)


class ToolAcquisitionTests(unittest.TestCase):
    def test_modified_download_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "download"
            path.write_bytes(b"modified release")
            expected = hashlib.sha256(b"official release").hexdigest()
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                acquire.verify_file(path, expected)

    def test_matching_download_is_verified(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "download"
            path.write_bytes(b"official release")
            expected = hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertEqual(acquire.verify_file(path, expected), expected)

    def test_archive_cannot_write_outside_destination(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            archive = base / "bad.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                member = tarfile.TarInfo("../outside")
                member.size = 3
                output.addfile(member, io.BytesIO(b"bad"))
            with self.assertRaises(ValueError):
                acquire.safe_extract(archive, base / "target")
            self.assertFalse((base / "outside").exists())

    def test_archive_cannot_create_escaping_symlink(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            archive = base / "link.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                member = tarfile.TarInfo("escape")
                member.type = tarfile.SYMTYPE
                member.linkname = "../../outside"
                output.addfile(member)
            with self.assertRaises(ValueError):
                acquire.safe_extract(archive, base / "target")

    def test_rust_component_installs_under_a_path_with_spaces(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "verified release" / "rustc-release"
            (package / "rustc/bin").mkdir(parents=True)
            (package / "components").write_text("rustc\n")
            (package / "rustc/bin/rustc").write_bytes(b"verified compiler fixture")
            destination = root / "T9 Data" / "local rust"
            acquire.install_rust_component(package.parent, destination)
            self.assertEqual((destination / "bin/rustc").read_bytes(), b"verified compiler fixture")


if __name__ == "__main__":
    unittest.main()
