"""A failed real discovery keeps diagnostics and never overwrites evidence."""
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('p18_phase4_discovery',ROOT/'crates/latexide-core/src/services/security/phase4_run.py')
runner = importlib.util.module_from_spec(spec); spec.loader.exec_module(runner)

class P18DiscoveryContract(unittest.TestCase):
    def test_failed_zero_and_duplicate_discovery_preserve_both_raw_logs(self):
        for output, code in [('failed output',2),('',0),('same: test\nsame: test\n',0)]:
            with self.subTest(code=code,output=output), tempfile.TemporaryDirectory() as d:
                root=Path(d)
                command=[sys.executable,'-c','import sys;sys.stdout.write(sys.argv[1]);sys.stderr.write("own diagnostic");raise SystemExit(int(sys.argv[2]))',output,str(code)]
                with self.assertRaises(ValueError): runner.discover_tests(command,root,root,os.environ.copy())
                self.assertEqual((root/'all-selection.stdout.log').read_bytes(),output.encode())
                self.assertEqual((root/'all-selection.stderr.log').read_bytes(),b'own diagnostic')

    def test_existing_discovery_log_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d); log=root/'all-selection.stdout.log';log.write_bytes(b'preserved failure')
            with self.assertRaises(FileExistsError):
                runner.discover_tests([sys.executable,'-c','print("one: test")'],root,root,os.environ.copy())
            self.assertEqual(log.read_bytes(),b'preserved failure')
