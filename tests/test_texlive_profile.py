import importlib.util
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('texlive_install',ROOT/'tools/texlive/install_local.py')
install=importlib.util.module_from_spec(spec);spec.loader.exec_module(install)

class PortableProfile(unittest.TestCase):
 def test_different_checkout_with_spaces_resolves_every_install_tree(self):
  prefix=Path('/private/tmp/Different checkout with spaces/.local/texlive-macos')
  profile=install.render_profile((ROOT/'tools/texlive/macos.profile').read_text(),prefix)
  values=dict(line.split(' ',1) for line in profile.splitlines() if line and not line.startswith('#'))
  self.assertEqual(values['TEXDIR'],str(prefix))
  for name in ['TEXMFLOCAL','TEXMFSYSCONFIG','TEXMFSYSVAR','TEXMFCONFIG','TEXMFVAR','TEXMFHOME']:
   self.assertTrue(values[name].startswith(str(prefix)+'/'),(name,values[name]))
  self.assertNotIn('/Volumes/T9 Data',profile)
  self.assertNotIn('@TEXLIVE_PREFIX@',profile)

if __name__=='__main__':unittest.main()
