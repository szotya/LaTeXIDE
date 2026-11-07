"""Own public C ABI regression for P03-P01-RUNTIME-F01.

Use the existing real C helper/sandbox/own-wait harness; never execute the
reviewer's runner. An outside target here is an own private synthetic sibling,
not a user file. Rejection must precede even an empty directory creation.
"""
import importlib.util
import os
from pathlib import Path
import stat
import sys

spec = importlib.util.spec_from_file_location('p01_own_controls', Path(__file__).with_name('controls.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


def outside_state(p):
    m = p.stat()
    return {'mode': stat.S_IMODE(m.st_mode), 'uid': m.st_uid,
            'entries': sorted(x.name for x in p.iterdir()),
            'sentinel': (p/'sentinel.txt').read_bytes()}


def project_and_outside(name, nested=False):
    base = c.RUN/name
    base.mkdir(mode=0o700)
    parent = base
    if nested:
        parent = base/'one'/'two'
        parent.mkdir(parents=True, mode=0o700)
    project = parent/'project'; project.mkdir(mode=0o755)
    (project/'main.tex').write_bytes(b'abc')
    (project/'fixture-nonce.txt').write_text(c.NONCE)
    outside = base/'outside'; outside.mkdir(mode=0o700)
    (outside/'sentinel.txt').write_text(c.NONCE)
    return project, outside


def reject_parent(nested=False):
    project, outside = project_and_outside('nested-parent' if nested else 'parent', nested)
    before = outside_state(outside)
    (project/'.latexide').symlink_to(outside, target_is_directory=True)
    c.Host(project, rejected=True)
    after = outside_state(outside)
    c.record(layer='actual_C_ABI_factory', control='symlink_parent_no_outside_write',
             outside=str(outside), before_entries=before['entries'], after_entries=after['entries'])
    assert after == before, 'Rejected C ABI initialization wrote through a symlink ancestor: '+repr(after)
    assert (project/'.latexide').is_symlink()


def reject_leaf():
    project, outside = project_and_outside('leaf')
    (project/'.latexide').mkdir(mode=0o700)
    (project/'.latexide/recovery').symlink_to(outside, target_is_directory=True)
    before = outside_state(outside)
    c.Host(project, rejected=True)
    assert outside_state(outside) == before
    assert (project/'.latexide/recovery').is_symlink()


def ordinary_new_existing_project_modes():
    project, outside = project_and_outside('normal')
    before = outside_state(outside)
    project_mode = stat.S_IMODE(project.stat().st_mode)
    for _ in range(2):
        h = c.Host(project)
        c.good(h.native(op='open_document', path='main.tex'))
        h.finish()
        assert stat.S_IMODE(project.stat().st_mode) == project_mode
        for p in [project/'.latexide', project/'.latexide/recovery']:
            assert stat.S_IMODE(p.stat().st_mode) == 0o700 and p.stat().st_uid == os.geteuid()
    assert outside_state(outside) == before


def invalid_existing_recovery_mode():
    project, outside = project_and_outside('mode')
    (project/'.latexide').mkdir(mode=0o700)
    store = project/'.latexide/recovery'; store.mkdir(mode=0o700)
    os.chmod(store, 0o755)
    before = outside_state(outside)
    c.Host(project, rejected=True)
    assert stat.S_IMODE(store.stat().st_mode) == 0o755
    assert list(store.iterdir()) == [] and outside_state(outside) == before


c.CONTROLS = [('test_symlink_parent_rejection_has_no_outside_write', reject_parent)]
if len(sys.argv)>3 and sys.argv[3]=='matrix':
    c.CONTROLS += [('test_leaf_symlink_rejection_has_no_outside_write', reject_leaf),
                   ('test_nested_project_parent_rejection_has_no_outside_write', lambda: reject_parent(True)),
                   ('test_normal_new_existing_private_paths_preserve_project_mode', ordinary_new_existing_project_modes),
                   ('test_existing_invalid_private_mode_rejected_without_chmod', invalid_existing_recovery_mode)]
if __name__=='__main__':
    sys.exit(c.main())
