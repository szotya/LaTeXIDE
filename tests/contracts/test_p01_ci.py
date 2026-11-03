"""Synthetic stdlib controls; no Rust/real parent-child qualification is run."""
from pathlib import Path
import importlib.util
import hashlib
import json
import os
import secrets
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from contextlib import contextmanager
from types import SimpleNamespace
from unittest.mock import patch

ROOT=Path(os.environ.get('P01_CI_MODULE_ROOT',Path(__file__).resolve().parents[2]))
def module(name):
    spec=importlib.util.spec_from_file_location('test_'+name,ROOT/'integration/registry'/f'{name}.py')
    m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def fixture_parent(directory=None):
    value=os.environ.get('P01_CI_TEST_FIXTURE') or directory or os.environ.get('TMPDIR')
    base=Path(value) if value else ROOT/'.local/p01-ci-python-fixtures'
    base.mkdir(parents=True,exist_ok=True,mode=0o700)
    return base

@contextmanager
def retained_directory(prefix='p01-fixture-',dir=None):
    # Keep failed/negative fixtures in the declared owner or this checkout's ignored root.
    base=fixture_parent(dir)
    yield tempfile.mkdtemp(prefix=prefix,dir=base)

# Independent literal model table, transcribed from the frozen source contract.
# Its files are tagged synthetic data and are never compiled or executed as Rust.
APPEND_FIXTURE = ("JournalWrite","JournalPartiallyWritten","JournalWritten","JournalFlushed",
    "JournalSync","JournalSynced","JournalParentSync","JournalParentSynced","BeforeAck","after_return")
CHECKPOINT_FIXTURE = ("CheckpointWrite","CheckpointPartiallyWritten","CheckpointWritten",
    "CheckpointFlushed","CheckpointSync","CheckpointSynced","CheckpointRename","CheckpointRenamed",
    "CheckpointParentSync","CheckpointParentSynced","JournalWrite","JournalPartiallyWritten",
    "JournalWritten","JournalFlushed","JournalSync","JournalSynced","JournalRename","JournalRenamed",
    "JournalParentSync","JournalParentSynced","BeforeAck","after_return")
RUNTIME_FIXTURE = ("edit_before_publish","edit_after_publish","save_intent_published",
                   "save_final_prepared","undo_after_save")

class Model:
    """Only external execution/source-version inputs are substituted, not validators."""
    def __init__(self, test, adapter=None, verifier=None):
        # Synthetic Cargo logs never consume the surrounding real TeX session.
        test.enterContext(patch.dict(os.environ,{k:v for k,v in os.environ.items()
            if k not in ('LATEXIDE_P18_PHASE2_ROOT','LATEXIDE_P18_PHASE3_ROOT',
                         'LATEXIDE_P18_TEX_PREFIX','LATEXIDE_P18_TEX_RUN')},clear=True))
        self.a=adapter or module('p01_ci');self.v=verifier
        base=fixture_parent()
        self.root=Path(tempfile.mkdtemp(prefix='synthetic-p01-',dir=base))
        self.calls=[];self.mode=None;self.extra_ids=[];self.extra_ignored=[];self.env={};self.normal_envs=[]
        self.all_ids=sorted([*self.a.EXCLUDED,'ordinary_contract'])
        self.run_id=secrets.token_hex(16)
        self.source_pins={}
        for name in self.a.SOURCE_PINS:
            p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True)
            p.write_bytes(b'SYNTHETIC OPAQUE MODEL DATA, NOT RUST SOURCE: '+name.encode())
            s=p.stat()
            self.source_pins[name]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),
                                  'bytes':s.st_size,'mode':stat.S_IMODE(s.st_mode)}
        test.enterContext(patch.object(self.a,'SOURCE_PINS',self.source_pins))
        if verifier:
            test.enterContext(patch.object(verifier.evidence.p01_ci,'SOURCE_PINS',self.source_pins))
        self.target=self.root/'target'
        self.binary=self.target/'debug/deps/recovery_contract-0123456789abcdef'
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b'SYNTHETIC INERT IMAGE; NEVER EXECUTED; NOT RUST')
        self.binary.chmod(0o700)
        self.binary_sha=hashlib.sha256(self.binary.read_bytes()).hexdigest()
        # Windows model tests represent POSIX stat facts at this one boundary.
        # Production _mode remains a real stat observation; no skip or relaxed
        # actual filesystem policy is introduced.
        if os.name=='nt':
            actual_mode=self.a._mode
            def model_mode(path):
                p=Path(path)
                if p.is_relative_to(self.root):
                    if p.is_dir():return 0o700
                    if p.name in ('COHORT','NAMESPACE.json'):return 0o600
                    if p==self.binary:return 0o700
                return actual_mode(p)
            test.enterContext(patch.object(self.a,'_mode',side_effect=model_mode))
            if verifier:test.enterContext(patch.object(verifier.evidence.p01_ci,'_mode',side_effect=model_mode))
        self.logs=self.root/'.local/run';self.logs.mkdir(parents=True,mode=0o700)
        self.log=self.logs/'01-rust-workspace.log';self.proof=self.logs/'p01.json'
        self.tool=self.a.binding(Path(sys.executable).resolve())
        self.argv=[str(Path(sys.executable).resolve()),'test','--workspace','--locked',
                   '--target-dir',str(self.target),'--message-format=json','--',
                   '--include-ignored','--test-threads=1',*self.a.skipped_arguments()]
        self.source_sha=hashlib.sha256(json.dumps(self.source_pins,sort_keys=True).encode()).hexdigest()
        self.record=None
        (self.root/'MODEL.json').write_text(json.dumps({'scope':'synthetic Python fixture only',
            'Rust_parent_child_processes':0,'nonce':os.environ.get('P01_CI_TEST_NONCE')}))

    def _table(self,kind,env):
        if kind=='durable':
            parent=Path(env['P01_DURABLE_SCRATCH'])/'tmp'
            nonce=env['P01_RUN_NONCE'];rows=[]
            for mode,phases in [('append',APPEND_FIXTURE),('checkpoint',CHECKPOINT_FIXTURE)]:
                for phase in phases:
                    d=parent/f'child-{mode}-{phase}';d.mkdir(mode=0o700)
                    (d/'child.log').write_text('running 1 test\ntest durable_contract::durable_crash_child ... ',encoding='utf-8',newline='\n')
                    trace=phases[:-1] if phase=='after_return' else phases[:phases.index(phase)+1]
                    (d/'phases.log').write_text('\n'.join(trace)+'\n',encoding='utf-8',newline='\n')
                    (d/'observed-r7.receipt').write_bytes(b'synthetic opaque ack bytes, not a real recovery receipt')
                    if phase=='after_return':(d/'observed-after-return.receipt').write_bytes(b'synthetic after-return bytes')
                    seq=8 if mode=='append' and phase=='after_return' else 7
                    rows.append(f'mode={mode} phase={phase} exit=73 observed_sequence={seq} recovered_sequence={seq} strict_rejected=false duration_ms=1 binary_sha256={self.binary_sha} nonce={nonce} fixture={d}\n')
            d=parent/'child-matrix-summary';d.mkdir(mode=0o700)
            (d/'32-process-boundary-cases.txt').write_text(''.join(rows),encoding='utf-8',newline='\n')
            return f'P01_CHILD_CASES=32 PASS=32 FAIL=0 ERROR=0 SKIP=0 nonce={nonce} evidence={d}'
        parent=Path(env['P01_RUNTIME_SCRATCH'])/'tmp'
        nonce=hashlib.sha256((self.run_id+':independent-runtime-model-nonce').encode()).hexdigest()
        rows=[]
        for i,case in enumerate(RUNTIME_FIXTURE):
            d=parent/f'runtime-{i}';d.mkdir(mode=0o700)
            (d/'project').mkdir();(d/'recovery').mkdir(mode=0o700)
            (d/'child.log').write_text('running 1 test\ntest runtime_contract::runtime_crash_child ... ',encoding='utf-8',newline='\n')
            rows.append(f'case={case} exit=73 duration_ms=1 binary_sha256={self.binary_sha} nonce={nonce} project={d}/project\n')
        summary=parent/'runtime-process-cases.txt';summary.write_text(''.join(rows),encoding='utf-8',newline='\n')
        return f'P01_RUNTIME_CHILD_CASES=5 PASS=5 FAIL=0 ERROR=0 nonce={nonce} evidence={summary}'

    def execute(self,name,argv,cwd,log,timeout,env=None):
        self.calls.append((name,list(argv),timeout))
        log=Path(log);log.parent.mkdir(parents=True,exist_ok=True)
        if name=='rust-workspace':
            artifact={'reason':'compiler-artifact','fixture_model':True,
              'target':{'name':'recovery_contract','src_path':str(self.root/'crates/latexide-core/tests/recovery_contract.rs')},
              'profile':{'test':True},'fresh':False,'executable':str(self.binary)}
            if self.mode=='cached':artifact['fresh']=True
            if self.mode=='fresh-bool-int':artifact['fresh']=0
            text='# SYNTHETIC PYTHON FIXTURE, NOT A RUST RUN\n'+json.dumps(artifact)+'\n'
            if self.env:text+='Running tests/security_policy_contract.rs (synthetic-inert-image)\n'
            ids=[n for n in self.all_ids if n not in self.a.EXCLUDED]
            text+=''.join('test '+n+' ... ok\n' for n in ids)
            if self.env:self.p18_outputs(self.env)  # Cargo captures successful test stdout.
            text+=f'test result: ok. {len(ids)} passed; 0 failed; 0 ignored; 0 measured; 4 filtered out;\n'
        elif name.startswith('normal-'):
            ids=[n for n in self.all_ids+self.extra_ids if n not in self.a.EXCLUDED]
            text='running '+str(len(ids))+' tests\n'+''.join('test '+n+' ... ok\n' for n in ids)
            if self.env:
                self.normal_envs.append(dict(env));text+=self.p18_outputs(env)
            text+=f'test result: ok. {len(ids)} passed; 0 failed; 0 ignored; 0 measured; 4 filtered out;\n'
        elif name.startswith('discovery'):
            ids=(list(self.a.CHILDREN.values())+self.extra_ignored if '--ignored' in argv
                 else self.all_ids+self.extra_ids)
            if self.mode=='zero-discovery':ids=[]
            text=''.join(n+': test\n' for n in ids)
        elif name in ['parent-durable','parent-runtime']:
            kind=name.removeprefix('parent-');marker=self._table(kind,env)
            if self.mode=='missing-parent-marker':marker=''
            text=f'running 1 test\ntest {self.a.PARENTS[kind]} ... {marker}\nok\n'
            text+=f'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {len(self.all_ids+self.extra_ids)-1} filtered out;\n'
        elif name=='late-model-change':
            self.child_log().write_text(self.child_log().read_text()+'\nsynthetic late mutation\n')
            text='synthetic file changed, no external provider executed\n'
        else:raise AssertionError('Unexpected model command: '+name)
        with log.open('x',encoding='utf-8',newline='\n') as f:f.write(text)
        row={'name':name,'command':list(argv),'status':'passed','exit_code':0,'timed_out':False,
             'duration_ms':1,'log':{'kind':'command_log','path':str(log),'sha256':hashlib.sha256(log.read_bytes()).hexdigest()},
             'fixture_execution_domain':'synthetic-Python-model'}
        if name=='rust-workspace':
            row['p18_environment']={key:self.env[key] for key in self.a.P18_ENV if key in self.env}
            self.record=row
        return row

    def ensure_record(self):
        if self.record is None:self.execute('rust-workspace',self.argv,self.root,self.log,10)
        return self.record

    def capture(self):
        self.a.qualify(self.root,self.ensure_record(),self.argv,self.run_id,self.source_sha,
                       self.target,self.proof,self.env,self.execute,self.tool)
        proof=json.loads(self.proof.read_bytes())
        archive=self.root/'capture-before-mutation.tar.gz'
        with tarfile.open(archive,'w:gz') as tar:
            for p in sorted(self.root.rglob('*')):
                if p!=archive:tar.add(p,arcname=p.relative_to(self.root).as_posix(),recursive=False)
        return proof

    def enable_p18(self):
        # Inert tool bytes and PDF placeholders exercise bindings only.
        prefix=self.root/'toolkit/texlive';prefix.mkdir(parents=True)
        (prefix/'inert-tool').write_bytes(b'synthetic toolkit; never executed')
        receipt={'schema':1,'prefix':str(prefix),'toolchain':module('p18_ci').toolchain_state(prefix)}
        (prefix.parent/'provision.json').write_text(json.dumps(receipt))
        self.env={'LATEXIDE_P18_TEX_PREFIX':str(prefix),'LATEXIDE_P18_TEX_RUN':'cargo-model'}
        for phase in (2,3):
            root=self.root/f'cargo-phase{phase}';root.mkdir(mode=0o700)
            (root/'artifacts').mkdir(mode=0o700)
            self.env[f'LATEXIDE_P18_PHASE{phase}_ROOT']=str(root)
        self.all_ids=sorted(self.all_ids+module('p18_ci').REQUIRED)

    def p18_outputs(self,env):
        markers=''
        for phase in (2,3):
            run=Path(env[f'LATEXIDE_P18_PHASE{phase}_ROOT'])/'artifacts'/env['LATEXIDE_P18_TEX_RUN']
            run.mkdir(mode=0o700)  # real exclusive-create collision, no validator stub
            for role in ('control','protected'):
                output=run/(role if phase==2 else role+'-generation')/'output';output.mkdir(parents=True)
                (output/'main.pdf').write_bytes(b'SYNTHETIC INERT PDF BINDING; NOT RENDERED')
            if phase==3:(run/'generation-publication.receipt').write_text('synthetic generation binding')
            markers+=f'P18_PHASE{phase}_TEX_ARTIFACTS={run}\n'
        return markers

    def validate(self,proof=None,run_id=None):
        if proof is not None:self.proof.write_text(json.dumps(proof))
        return self.a.validate(self.root,self.proof,self.record,self.argv,run_id or self.run_id,
                               self.source_sha,self.target,self.tool)

    def child_log(self):
        d=json.loads(self.proof.read_bytes())
        return Path(d['observations'][0]['cases'][0]['files'][0]['path'])

    def rust_step(self):
        return {'name':'rust-workspace','kind':'rust-unit','summary_format':'rust',
                'argv':self.argv,'p01_cohort':{'proof':str(self.proof)},
                'artifact_paths':[{'path':str(self.proof),'kind':'p01_cohort_proof'},
                                  {'path':str(self.log),'kind':'p01_rust_build_log'}],
                'required_artifact_kinds':['p01_cohort_proof','p01_rust_build_log'],
                'platform':'synthetic/model'}

    def driver_report(self,late=False):
        steps=[self.rust_step()]
        if late:steps.append({'name':'late-model-change','kind':'source','argv':[sys.executable,'model-only']})
        with patch.object(self.v,'execute_step',side_effect=self.execute):
            report=self.v.run_checks(self.root,list(self.source_pins),steps,self.logs,10)
        report['selection']={'platform':'synthetic','machine':'model'}
        return report,steps

class P01NamespaceTests(unittest.TestCase):
    def test_synthetic_process_transcripts_keep_LF_under_windows_translation(self):
        m=Model(self);actual=Path.open
        def windows_text_open(path,mode='r',*args,**kwargs):
            if 'b' not in mode and any(flag in mode for flag in 'wax') and kwargs.get('newline') is None:
                kwargs['newline']='\r\n'
            return actual(path,mode,*args,**kwargs)
        with patch.object(Path,'open',windows_text_open):
            m.capture();m.validate()
        self.assertNotIn(b'\r\n',m.log.read_bytes())

    def test_inherited_cargo_scope_allows_captured_stdout_and_other_image_names(self):
        m=Model(self);m.enable_p18();m.ensure_record()
        raw=m.log.read_text()
        raw='\n'.join(line for line in raw.splitlines() if not line.startswith('P18_PHASE'))+'\n'
        for target in ('other-one','other-two'):
            raw+=f'Running tests/{target}.rs (inert-model-image)\ntest review_focus_r2 ... ok\n'
            raw+='test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n'
        inherited=m.a._p18_inherited(m.env,raw)
        self.assertEqual(inherited['environment'],m.env)

    def test_replay_uses_fresh_namespace_after_cargo_exclusive_create(self):
        m=Model(self);m.enable_p18();m.ensure_record()
        original={p:hashlib.sha256(p.read_bytes()).hexdigest() for p in m.root.glob('cargo-phase*/artifacts/**/*') if p.is_file()}
        p=m.capture();m.validate()
        self.assertEqual(len(m.normal_envs),1)
        for env in m.normal_envs:
            self.assertNotEqual(env['LATEXIDE_P18_TEX_RUN'],m.env['LATEXIDE_P18_TEX_RUN'])
            for phase in (2,3):
                root=Path(env[f'LATEXIDE_P18_PHASE{phase}_ROOT'])
                self.assertTrue(root.is_relative_to(Path(p['scratch'])))
                self.assertNotEqual(root,Path(m.env[f'LATEXIDE_P18_PHASE{phase}_ROOT']))
        self.assertEqual(original,{path:hashlib.sha256(path.read_bytes()).hexdigest() for path in original})

    def test_stale_foreign_or_missing_namespace_bindings_fail_read_only(self):
        for fault in ('missing-contract','missing-namespace','stale-label','foreign-root','escape',
                      'wrong-nonce','wrong-source','wrong-binary','foreign-proof','wrong-directory','fifth-skip',
                      'wrong-cargo-env','missing-cargo-env'):
            with self.subTest(fault=fault):
                m=Model(self);m.enable_p18();p=m.capture();calls=len(m.calls)
                row=p['normal'][0];ns=row['p18_namespace']
                if fault=='missing-contract':p['p18']=None
                elif fault=='missing-namespace':row.pop('p18_namespace')
                elif fault=='stale-label':ns['environment']['LATEXIDE_P18_TEX_RUN']='cargo-model'
                elif fault=='foreign-root':ns['environment']['LATEXIDE_P18_PHASE2_ROOT']=m.env['LATEXIDE_P18_PHASE2_ROOT']
                elif fault=='escape':ns['environment']['LATEXIDE_P18_PHASE2_ROOT']=str(m.root/'../foreign')
                elif fault=='wrong-nonce':row['context_binding']['driver_nonce']='0'*64
                elif fault=='wrong-source':row['context_binding']['source_sha256']='0'*64
                elif fault=='wrong-binary':row['context_binding']['binary']['sha256']='0'*64
                elif fault=='foreign-proof':ns['metadata']['path']=str(m.root/'toolkit/provision.json')
                elif fault=='wrong-directory':ns['directories'][0]['inode']+=1
                elif fault=='fifth-skip':row['command']+=['--skip','os_subprocess_boundary_probe']
                elif fault=='wrong-cargo-env':m.record['p18_environment']['LATEXIDE_P18_TEX_RUN']='foreign'
                else:m.record.pop('p18_environment')
                with self.assertRaises(ValueError):m.validate(p)
                self.assertEqual(len(m.calls),calls)

    def test_namespace_metadata_is_rederived_from_context_and_argv(self):
        for field in ('driver_nonce','source_sha256','binary','phase','verification_run_id','command','inherited_sha256'):
            with self.subTest(field=field):
                m=Model(self);m.enable_p18();p=m.capture()
                path=Path(p['normal'][0]['p18_namespace']['metadata']['path'])
                data=json.loads(path.read_bytes());data['identity'][field]='foreign'
                path.write_text(json.dumps(data))
                # Even a refreshed file hash cannot authorize another invocation.
                p['normal'][0]['p18_namespace']['metadata']=m.a.binding(path)
                with self.assertRaisesRegex(ValueError,'source/binary/nonce/argv/proof'):m.validate(p)

    def test_missing_raw_phase_proof_fails_with_refreshed_log_hash(self):
        m=Model(self);m.enable_p18();p=m.capture();row=p['normal'][0];log=Path(row['log']['path'])
        raw=log.read_text();log.write_text('\n'.join(line for line in raw.splitlines() if not line.startswith('P18_PHASE2_'))+'\n')
        row['log']['sha256']=hashlib.sha256(log.read_bytes()).hexdigest()
        (Path(p['scratch'])/'normal-0.json').write_text(json.dumps(row))
        with self.assertRaisesRegex(ValueError,'Missing/stale P18 invocation proof'):m.validate(p)

    def test_namespace_symlink_and_missing_pdf_proof_fail(self):
        for fault in ('symlink','missing-pdf','toolkit-change','missing-provision'):
            with self.subTest(fault=fault):
                m=Model(self);m.enable_p18();p=m.capture()
                phase=Path(p['normal'][0]['p18_namespace']['environment']['LATEXIDE_P18_PHASE2_ROOT'])
                if fault=='symlink':
                    retained=phase.with_name('phase2-retained');phase.rename(retained)
                    try:phase.symlink_to(retained,target_is_directory=True)
                    except OSError as error:
                        if os.name!='nt':raise
                        retained.rename(phase)
                        # Windows models the actual link observation at the stat boundary.
                        actual=Path.is_symlink
                        with patch.object(Path,'is_symlink',lambda path: path==phase or actual(path)):
                            with self.assertRaises((OSError,ValueError)):m.validate(p)
                        continue
                elif fault=='missing-pdf':
                    pdf=next(phase.rglob('main.pdf'));pdf.rename(pdf.with_name('retained.pdf'))
                elif fault=='toolkit-change':(Path(m.env['LATEXIDE_P18_TEX_PREFIX'])/'inert-tool').write_bytes(b'changed')
                else:
                    receipt=Path(m.env['LATEXIDE_P18_TEX_PREFIX']).parent/'provision.json'
                    receipt.rename(receipt.with_name('retained-provision.json'))
                with self.assertRaises((OSError,ValueError)):m.validate(p)

    def test_existing_invocation_directory_is_preserved_and_refused(self):
        m=Model(self);m.enable_p18();execute=m.execute;retained=[]
        def collide(name,*args,**kwargs):
            row=execute(name,*args,**kwargs)
            if name=='discovery-ignored-0':
                directory=Path(row['log']['path']).parent/'p18-normal-0';directory.mkdir(mode=0o700)
                marker=directory/'retained';marker.write_bytes(b'old invocation');retained.append(marker)
            return row
        with patch.object(m,'execute',side_effect=collide),self.assertRaises(FileExistsError):m.capture()
        self.assertEqual(retained[0].read_bytes(),b'old invocation')
        self.assertEqual(m.normal_envs,[])

    def test_second_image_gets_distinct_private_roots_label_and_receipt(self):
        m=Model(self);m.enable_p18();p=m.capture();image=dict(p['images'][0])
        binary=m.binary.with_name('second-image');binary.write_bytes(b'other inert image');binary.chmod(0o700)
        image['binary']=m.a.binding(binary);command=m.a.normal_command(image,[])
        ns=m.a._p18_namespace(Path(p['scratch']),'normal-1',image,p['context'],p['driver_nonce'],command,p['p18'],create=True)
        first=p['normal'][0]['p18_namespace']
        for key in ('LATEXIDE_P18_PHASE2_ROOT','LATEXIDE_P18_PHASE3_ROOT','LATEXIDE_P18_TEX_RUN'):
            self.assertNotEqual(first['environment'][key],ns['environment'][key])
        self.assertNotEqual(first['metadata']['inode'],ns['metadata']['inode'])
        with self.assertRaises(FileExistsError):
            m.a._p18_namespace(Path(p['scratch']),'normal-1',image,p['context'],p['driver_nonce'],command,p['p18'],create=True)

    def test_final_seal_detects_added_output_after_normal_observation(self):
        m=Model(self);m.enable_p18();p=m.capture()
        output=p['normal_observations'][0]['p18_namespace']['outputs']['2']
        (Path(output['directories'][0]['path'])/'late-proof').write_bytes(b'late foreign evidence')
        with self.assertRaisesRegex(ValueError,'Late P18 output inventory changed'):m.a._normal_final(p)

class P01CiTests(unittest.TestCase):
    def test_parent_only_final_rebinds_receipt_even_if_json_value_is_unchanged(self):
        a=module('p01_ci')
        with retained_directory(prefix='late-receipt-') as d:
            root=Path(d);scratch=root/'scratch';scratch.mkdir();(scratch/'child.log').write_text('fixture')
            compiler=root/'compiler.json';compiler.write_text('{}\n')
            fixture=root/'fixture.json';fixture.write_text(json.dumps(a._fixture_inventory(scratch)))
            proof={'compiler':a.binding(compiler),'fixture':a.binding(fixture)}
            fixture.write_text(fixture.read_text()+'\n')
            with self.assertRaises(ValueError):a._parent_only_final(proof,scratch)

    def test_child_header_uses_source_role_without_claiming_a_raw_entry_ID(self):
        a=module('p01_ci')
        self.assertFalse(a.child_entry_evidence('\nrunning 1 test\n',a.CHILDREN['durable']))

    def test_child_missing_and_unknown_transcripts_are_still_rejected(self):
        a=module('p01_ci')
        for raw in ['', 'running 0 tests\n','test unknown_child ... ']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):a.child_entry_evidence(raw,a.CHILDREN['durable'])

    def test_child_success_summary_is_rejected_even_with_the_expected_ID(self):
        a=module('p01_ci');line='\nrunning 1 test\ntest '+a.CHILDREN['durable']+' ... ok\n'
        for raw in [line,line+'test result: ok. 1 passed;\n']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):a.child_entry_evidence(raw,a.CHILDREN['durable'])

    def test_parent_only_depfile_preserves_escaped_spaces_and_relative_sources(self):
        a=module('p01_ci')
        raw='out.d: /own/T9\\ Data/src.rs crates/core/tests/../src/mod.rs\n/own/T9\\ Data/src.rs:\n'
        self.assertEqual(a.depfile_inputs(raw),['/own/T9 Data/src.rs','crates/core/tests/../src/mod.rs'])

    def test_parent_only_depfile_rejects_empty_or_truncated_input(self):
        a=module('p01_ci')
        for raw in ['# no prerequisites\n','out: trailing'+'\\','no-colon']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):a.depfile_inputs(raw)

    def test_parent_only_rejects_workspace_selection_before_any_process_or_source_read(self):
        a=module('p01_ci');argv=['cargo','test','--workspace']
        with self.assertRaisesRegex(ValueError,'Wrong scoped build selection'):
            a._parent_only_context(ROOT,{'command':argv},argv,'a'*32,{'target':'unrun','tool_root':'unrun'})

    def test_standalone_private_child_pass_is_not_acceptance(self):
        evidence=module('evidence')
        with retained_directory(prefix='p01-red-') as d:
            log=Path(d)/'rust.log';log.write_text('running 1 test\ntest durable_contract::durable_crash_child ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n')
            with self.assertRaises(ValueError):evidence.rust_summary(log,'rust-workspace')

    def test_other_private_child_ignored_is_not_an_allowlist(self):
        e=module('evidence')
        with retained_directory() as d:
            p=Path(d)/'log';p.write_text('test runtime_contract::runtime_crash_child ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored;\n')
            with self.assertRaises(ValueError):e.rust_summary(p,'rust-workspace')

    def test_main_first_slice_is_not_a_fake_zero_case_pass(self):
        a=module('p01_ci')
        with retained_directory() as d:self.assertIsNone(a.catalog(d))

    def test_partial_provider_source_is_rejected(self):
        m=Model(self);(m.root/m.a.SENTINELS[1]).unlink()
        with self.assertRaises(ValueError):m.a.catalog(m.root)

    def test_unknown_parent_source_version_is_rejected(self):
        m=Model(self);p=m.root/m.a.SENTINELS[0];p.write_text('different opaque source data')
        with self.assertRaises(ValueError):m.a.catalog(m.root)

    def test_main_mac_include_ignored_serial_selection_is_preserved(self):
        v=module('verify')
        with retained_directory() as d:
            root=Path(d);args=SimpleNamespace(profile='shared',run_dir=root/'.local/run',cargo_executable=None,offline=True,native_schema=False)
            steps=v.build_steps(root,args,'darwin','arm64');r=next(x for x in steps if x['name']=='rust-workspace')
            self.assertEqual(r['argv'][-3:],['--','--include-ignored','--test-threads=1'])
            self.assertNotIn('p01_cohort',r);self.assertIn('p18-compiler-pdfs',[x['name'] for x in steps])

    def test_provider_plan_has_explicit_four_exclusions_and_bound_proof(self):
        v=module('verify');m=Model(self,v.p01_ci,v)
        args=SimpleNamespace(profile='integrated',run_dir=m.logs,cargo_executable=sys.executable,offline=True,native_schema=False)
        steps=v.build_steps(m.root,args,'darwin','arm64');r=next(x for x in steps if x['name']=='rust-workspace')
        self.assertIn('--message-format=json',r['argv']);self.assertEqual(r['argv'][-8:],m.a.skipped_arguments())
        self.assertEqual(set(r['required_artifact_kinds']),{'p18_os_probe','p01_cohort_proof','p01_rust_build_log'})
        self.assertIn('p18-compiler-pdfs',next(x for x in steps if x['name']=='runtime-build')['depends_on'])

    def test_non_mac_profile_does_not_claim_p01_cohort_qualification(self):
        v=module('verify');m=Model(self,v.p01_ci,v)
        args=SimpleNamespace(profile='shared',run_dir=m.logs,cargo_executable=None,offline=True,native_schema=False)
        r=next(x for x in v.build_steps(m.root,args,'win32','AMD64') if x['name']=='rust-workspace')
        self.assertNotIn('p01_cohort',r);self.assertNotIn('--skip',r['argv'])

    def test_model_parents_and_helpers_and_subcases_are_separate_units(self):
        m=Model(self);proof=m.capture();q=m.validate()
        self.assertEqual((q['parent_methods'],q['helper_entries'],q['parent_subcases'],q['child_passes']),(2,2,37,0))
        normal={'schema':1,'suite':'rust-workspace','tests':1,'passed':1,'skipped':0,'failures':0,'errors':0,'skipped_ids':[]}
        self.assertEqual(m.a.combine_summary(normal,q)['tests'],3)
        self.assertNotEqual(proof['observations'][0]['nonce'],proof['observations'][1]['nonce'])

    def test_readonly_validation_never_invokes_driver(self):
        m=Model(self);m.capture();before=len(m.calls)
        with patch.object(m.a,'qualify',side_effect=AssertionError('must not execute')):self.assertEqual(m.validate()['parent_methods'],2)
        self.assertEqual(len(m.calls),before)

    def test_wrong_parent_command_is_rejected(self):
        m=Model(self);p=m.capture();p['parents']['runtime']['command'][1]='other_parent'
        with self.assertRaises(ValueError):m.validate(p)

    def test_bool_exit_is_not_integer_zero(self):
        m=Model(self);m.ensure_record();m.record['exit_code']=False
        with self.assertRaises(ValueError):m.capture()
        self.assertFalse(any(n.startswith('parent-') for n,_,_ in m.calls))

    def test_parent_bool_wait_is_rejected(self):
        m=Model(self);p=m.capture();p['parents']['runtime']['duration_ms']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_parent_unbounded_wait_is_rejected(self):
        m=Model(self);p=m.capture();p['parents']['runtime']['duration_ms']=10001
        with self.assertRaises(ValueError):m.validate(p)

    def test_parent_timeout_cannot_pass(self):
        m=Model(self);p=m.capture();p['parents']['runtime']['timed_out']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_missing_case_is_rejected(self):
        m=Model(self);p=m.capture();f=Path(p['observations'][0]['table']['path']);f.write_text(''.join(f.read_text().splitlines(keepends=True)[:-1]))
        with self.assertRaises(ValueError):m.validate()

    def test_case_nonce_mismatch_is_rejected(self):
        m=Model(self);p=m.capture();f=Path(p['observations'][0]['table']['path'])
        f.write_text(f.read_text().replace(p['driver_nonce'],'0'*64))
        with self.assertRaises(ValueError):m.validate()

    def test_case_binary_mismatch_is_rejected(self):
        m=Model(self);p=m.capture();f=Path(p['observations'][0]['table']['path']);f.write_text(f.read_text().replace(m.binary_sha,'0'*64))
        with self.assertRaises(ValueError):m.validate()

    def test_zero_discovery_never_starts_parent(self):
        m=Model(self);m.mode='zero-discovery'
        with self.assertRaises(ValueError):m.capture()
        self.assertFalse(any(n.startswith('parent-') for n,_,_ in m.calls))

    def test_unknown_ignored_helper_never_starts_parent(self):
        m=Model(self);m.extra_ids=['new_helper'];m.extra_ignored=['new_helper']
        with self.assertRaises(ValueError):m.capture()
        self.assertFalse(any(n.startswith('parent-') for n,_,_ in m.calls))

    def test_skip_substring_collision_is_rejected(self):
        m=Model(self);m.extra_ids=['other::durable_contract::durable_crash_child_extra']
        with self.assertRaises(ValueError):m.capture()

    def test_cached_binary_is_not_a_new_cohort(self):
        m=Model(self);m.mode='cached'
        with self.assertRaises(ValueError):m.capture()
        self.assertFalse(any(n.startswith('parent-') for n,_,_ in m.calls))

    def test_cargo_fresh_integer_is_not_boolean_false(self):
        m=Model(self);m.mode='fresh-bool-int'
        with self.assertRaises(ValueError):m.capture()

    def test_binary_mutation_is_rejected(self):
        m=Model(self);m.capture();m.binary.write_bytes(b'new inert data')
        with self.assertRaises(ValueError):m.validate()

    def test_identical_binary_object_replacement_is_rejected(self):
        m=Model(self);m.capture();p=m.binary.with_suffix('.new');p.write_bytes(m.binary.read_bytes());p.chmod(0o700);p.replace(m.binary)
        with self.assertRaises(ValueError):m.validate()

    def test_stale_step_nonce_is_rejected(self):
        m=Model(self);m.capture()
        with self.assertRaises(ValueError):m.validate(run_id='0'*32)

    def test_nonce_marker_tamper_is_rejected(self):
        m=Model(self);p=m.capture();(Path(p['scratch'])/'COHORT').write_text('0'*64)
        with self.assertRaises(ValueError):m.validate()

    def test_identical_nonce_marker_object_replacement_is_rejected(self):
        m=Model(self);p=m.capture();old=Path(p['scratch'])/'COHORT';new=old.with_suffix('.new')
        new.write_bytes(old.read_bytes());new.chmod(0o600);new.replace(old)
        with self.assertRaises(ValueError):m.validate()

    def test_child_log_mutation_is_rejected(self):
        m=Model(self);m.capture();m.child_log().write_text(m.child_log().read_text()+'\nchanged')
        with self.assertRaises(ValueError):m.validate()

    def test_child_pass_cannot_replace_parent_case(self):
        m=Model(self);m.capture();m.child_log().write_text(m.child_log().read_text()+'\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n')
        with self.assertRaises(ValueError):m.validate()

    def test_phase_transcript_mismatch_is_rejected(self):
        m=Model(self);p=m.capture();f=Path(p['observations'][0]['cases'][0]['files'][1]['path']);f.write_text('UnknownPhase\n')
        with self.assertRaises(ValueError):m.validate()

    def test_unknown_runtime_case_is_rejected(self):
        m=Model(self);p=m.capture();f=Path(p['observations'][1]['table']['path'])
        f.write_text(f.read_text().replace('edit_before_publish','unknown_case'))
        with self.assertRaises(ValueError):m.validate()

    def test_bool_schema_is_not_version_one(self):
        m=Model(self);p=m.capture();p['schema']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_bool_zero_child_pass_count_is_rejected(self):
        a=module('p01_ci');normal={'tests':1,'passed':1,'skipped':0,'failures':0,'errors':0}
        q={'parent_methods':2,'helper_entries':2,'child_passes':False,'parent_subcases':37}
        with self.assertRaises(ValueError):a.combine_summary(normal,q)

    def test_unknown_proof_field_is_rejected(self):
        m=Model(self);p=m.capture();p['allow_skip']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_proof_quota_is_fail_closed(self):
        m=Model(self);m.capture();m.proof.write_bytes(b' '*(1024*1024+1))
        with self.assertRaises(ValueError):m.validate()

    def test_wrong_process_cwd_is_rejected(self):
        m=Model(self);p=m.capture();p['parents']['runtime']['cwd']=str(m.root/'other')
        with self.assertRaises(ValueError):m.validate(p)

    def test_wrong_owned_environment_is_rejected(self):
        m=Model(self);p=m.capture();p['parents']['runtime']['owned_environment']['P01_RUN_NONCE']='0'*64
        with self.assertRaises(ValueError):m.validate(p)

    def test_source_metadata_bool_int_is_rejected(self):
        m=Model(self);p=m.capture();p['source_before'][next(iter(p['source_before']))]['bytes']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_reusing_same_claim_cannot_overwrite_evidence(self):
        m=Model(self);m.capture();before=m.proof.read_bytes()
        with self.assertRaises(FileExistsError):m.capture()
        self.assertEqual(m.proof.read_bytes(),before)

    def test_step_driver_model_and_readonly_parent_receipt_binding(self):
        v=module('verify');m=Model(self,v.p01_ci,v);r,steps=m.driver_report()
        self.assertEqual(r['status'],'passed');self.assertEqual(r['checks'][0]['test_summary']['tests'],3)
        current=v.source_identity(m.root,list(m.source_pins))
        checked=v.evidence.revalidate_report(m.root,r,current,steps)
        self.assertEqual(checked['status'],'current')

    def test_cargo_exit_zero_without_parent_marker_fails_driver(self):
        v=module('verify');m=Model(self,v.p01_ci,v);m.mode='missing-parent-marker';r,_=m.driver_report()
        self.assertEqual(r['status'],'failed');self.assertNotEqual(r['checks'][0]['status'],'passed')
        self.assertFalse(m.proof.exists())

    def test_step_finalization_revalidates_nested_parent_files(self):
        v=module('verify');m=Model(self,v.p01_ci,v);r,_=m.driver_report(late=True)
        self.assertEqual(r['status'],'failed');self.assertEqual(r['checks'][0]['status'],'stale_artifact')

    def test_readonly_final_recheck_catches_late_nested_mutation(self):
        v=module('verify');m=Model(self,v.p01_ci,v);r,steps=m.driver_report()
        self.assertEqual(r['status'],'passed')
        current=v.source_identity(m.root,list(m.source_pins));last=m.root/current['files'][-1]['path']
        original=v.evidence.digest;armed=[]
        def late(path):
            result=original(path)
            if Path(path)==last and not armed:
                armed.append(True);m.child_log().write_text(m.child_log().read_text()+'\nlate byte change\n')
            return result
        with patch.object(v.evidence,'digest',side_effect=late):
            checked=v.evidence.revalidate_report(m.root,r,current,steps)
        self.assertTrue(armed);self.assertNotEqual(checked['status'],'current')

    def test_missing_parent_build_tool_is_unrun_not_success(self):
        v=module('verify');m=Model(self,v.p01_ci,v);step=m.rust_step();step['argv']=[str(m.root/'missing-cargo'),*m.argv[1:]]
        with patch.object(v,'execute_step',side_effect=AssertionError('missing tool must not run')):
            r=v.run_checks(m.root,list(m.source_pins),[step],m.logs,10)
        self.assertEqual(r['checks'][0]['status'],'unrun');self.assertEqual(r['status'],'failed')

    def test_bound_fixture_does_not_require_tmpdir_environment(self):
        errors=[]
        with retained_directory(prefix='portable-fixture-owner-') as owner:
            with patch.dict(os.environ,{'P01_CI_TEST_FIXTURE':owner}):
                os.environ.pop('TMPDIR',None)
                try:
                    with retained_directory(prefix='no-tmpdir-') as d:
                        self.assertEqual(Path(d).parent,Path(owner))
                        self.assertEqual(Model(self).root.parent,Path(owner))
                except KeyError as caught:errors.append(('bound',str(caught)))
            with patch.dict(os.environ),patch.dict(globals(),ROOT=Path(owner)):
                os.environ.pop('TMPDIR',None);os.environ.pop('P01_CI_TEST_FIXTURE',None)
                try:
                    with retained_directory(prefix='own-default-') as d:
                        self.assertEqual(Path(d).parent,Path(owner)/'.local/p01-ci-python-fixtures')
                except KeyError as caught:errors.append(('own-default',str(caught)))
        self.assertEqual(errors,[],'Fixture setup must not require TMPDIR or use an external fallback')

    def test_python_cli_source_inspection_does_not_run_provider(self):
        a=module('p01_ci')
        with retained_directory() as d:
            r=subprocess.run([sys.executable,'-B',a.__file__,'--inspect',d],capture_output=True,text=True,timeout=5)
            self.assertEqual(r.returncode,0);data=json.loads(r.stdout)
            self.assertIs(data['execution_performed'],False);self.assertIs(data['applicable'],False)



class CombinedIntegrationTests(unittest.TestCase):
    """New cross-boundary controls: actual-style headers plus strict normal evidence."""
    def fixture(self, mode=None):
        m=Model(self)
        before=m._table
        def table(kind,env):
            value=before(kind,env)
            parent=Path(env['P01_DURABLE_SCRATCH'] if kind=='durable' else env['P01_RUNTIME_SCRATCH'])/'tmp'
            for p in parent.rglob('child.log'):p.write_bytes(b'\nrunning 1 test\n')
            return value
        m._table=table
        execute=m.execute
        def run(name,argv,cwd,log,timeout,env=None):
            row=execute(name,argv,cwd,log,timeout,env=env)
            p=Path(log);data=p.read_text()
            if mode=='normal-ID' and name=='normal-0':data=data.replace('ordinary_contract','substituted_contract')
            if mode=='Cargo-ID' and name=='rust-workspace':data=data.replace('test ordinary_contract','test substituted_contract')
            if name.startswith('parent-') and mode in {'FAILED','missing','ignored'}:
                data=data.replace('\nok\n','\n'+({'FAILED':'FAILED','missing':'','ignored':'ignored'}[mode])+'\n')
            p.write_text(data);row['log']['sha256']=hashlib.sha256(p.read_bytes()).hexdigest()
            return row
        m.execute=run
        return m

    def test_B01_headers_with_exact_normal_identity_complete_one_bound_flow(self):
        m=self.fixture();p=m.capture();q=m.validate()
        self.assertEqual(q['normal_methods'],1)
        self.assertEqual([x['method_id'] for x in q['normal_identities']],['ordinary_contract'])
        self.assertEqual([o['subcases'] for o in p['observations']],[32,5])
        self.assertTrue(all(c['raw_child_ID_observed'] is False for o in p['observations'] for c in o['cases']))
        self.assertEqual(len(m.calls),6)

    def test_B02_header_role_cannot_launder_substituted_normal_ID(self):
        with self.assertRaises(ValueError):self.fixture('normal-ID').capture()

    def test_B03_header_role_cannot_launder_wrong_Cargo_multiset(self):
        with self.assertRaises(ValueError):self.fixture('Cargo-ID').capture()

    def test_B04_header_child_does_not_complete_FAILED_parent(self):
        with self.assertRaises(ValueError):self.fixture('FAILED').capture()

    def test_B05_header_child_does_not_complete_missing_parent_terminal(self):
        with self.assertRaises(ValueError):self.fixture('missing').capture()

    def test_B06_header_child_does_not_complete_ignored_parent(self):
        with self.assertRaises(ValueError):self.fixture('ignored').capture()

    def test_B07_rehash_normal_record_does_not_refresh_sealed_snapshot(self):
        m=self.fixture();p=m.capture();row=p['normal'][0];log=Path(row['log']['path'])
        log.write_text(log.read_text()+'\nretagged raw\n');row['log']['sha256']=hashlib.sha256(log.read_bytes()).hexdigest()
        with self.assertRaises(ValueError):m.validate(p)

    def test_B08_header_flow_normal_snapshot_keeps_exact_JSON_types(self):
        m=self.fixture();p=m.capture();p['normal_observations'][0]['log']['bytes']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_B09_header_flow_run_identity_cannot_be_boolean(self):
        m=self.fixture();p=m.capture();p['normal'][0]['context_binding']['verification_run_id']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_B10_header_flow_binary_permissions_cannot_be_equal_valued_float(self):
        m=self.fixture();p=m.capture();b=p['normal'][0]['context_binding']['binary'];b['mode']=float(b['mode'])
        with self.assertRaises(ValueError):m.validate(p)

    def test_B11_header_flow_normal_log_cannot_alias_discovery_partition(self):
        m=self.fixture();p=m.capture();p['normal'][0]['log']=p['discovery'][0]['all']['log']
        with self.assertRaises(ValueError):m.validate(p)

    def test_B12_header_flow_parent_cannot_borrow_normal_phase(self):
        m=self.fixture();p=m.capture();p['parents']['runtime']['context_binding']=p['normal'][0]['context_binding']
        with self.assertRaises(ValueError):m.validate(p)

    def test_B13_rehashed_unknown_bytes_are_not_header_role_evidence(self):
        m=self.fixture();p=m.capture();case=p['observations'][0]['cases'][0];log=Path(case['files'][0]['path'])
        log.write_bytes(log.read_bytes()+b'unknown entry bytes\n');case['files'][0]=m.a.binding(log)
        with self.assertRaises(ValueError):m.validate(p)

    def test_B14_header_only_snapshot_cannot_claim_observed_child_ID(self):
        m=self.fixture();p=m.capture();p['observations'][0]['cases'][0]['raw_child_ID_observed']=True
        with self.assertRaises(ValueError):m.validate(p)

    def test_B15_late_coordinated_raw_and_proof_retagging_stays_invalid(self):
        m=self.fixture();p=m.capture();original=m.a.binding;tool_calls=[];armed=[]
        def late(path):
            if Path(path)==Path(m.tool['path']):
                tool_calls.append(True)
                if len(tool_calls)==3:
                    log=Path(p['normal'][0]['log']['path']);log.write_text(log.read_text()+'\ncoordinated late bytes\n')
                    p['normal'][0]['log']['sha256']=hashlib.sha256(log.read_bytes()).hexdigest()
                    p['normal_observations'][0]['log']=original(log)
                    m.proof.write_text(json.dumps(p));armed.append(True)
            return original(path)
        with patch.object(m.a,'binding',side_effect=late),self.assertRaises(ValueError):m.validate()
        self.assertTrue(armed)

    def test_B16_same_bytes_new_normal_log_object_is_not_same_observation(self):
        m=self.fixture();p=m.capture();log=Path(p['normal'][0]['log']['path'])
        replacement=log.with_suffix('.new');replacement.write_bytes(log.read_bytes());replacement.chmod(stat.S_IMODE(log.stat().st_mode))
        replacement.replace(log)
        with self.assertRaises(ValueError):m.validate()

    def test_B17_header_flow_cannot_select_private_child_in_normal_argv(self):
        m=self.fixture();p=m.capture();p['normal'][0]['command']=[str(m.binary),m.a.CHILDREN['durable'],'--exact']
        with self.assertRaises(ValueError):m.validate(p)

    def test_B18_header_flow_normal_source_digest_is_phase_bound(self):
        m=self.fixture();p=m.capture();p['normal'][0]['context_binding']['source_sha256']='0'*64
        with self.assertRaises(ValueError):m.validate(p)

    def test_B19_rehashed_child_terminal_without_summary_is_still_not_coverage(self):
        m=self.fixture();p=m.capture();case=p['observations'][1]['cases'][0];log=Path(case['files'][0]['path'])
        log.write_text('\nrunning 1 test\ntest '+m.a.CHILDREN['runtime']+' ... ok\n');case['files'][0]=m.a.binding(log)
        with self.assertRaises(ValueError):m.validate(p)

    def test_B20_old_private_schema_is_refused_without_reexecution_or_upgrade(self):
        m=self.fixture();p=m.capture();p['schema']=1;m.proof.write_text(json.dumps(p));before=m.proof.read_bytes();calls=len(m.calls)
        with self.assertRaises(ValueError):m.validate()
        self.assertEqual(m.proof.read_bytes(),before);self.assertEqual(len(m.calls),calls)


    def actual_link_fixture(self):
        a=module('p01_ci');root=Path(self.enterContext(retained_directory(prefix='negative-link-custody-')))
        pid=12345
        targets={
            f'durable/tmp/path-fixture-{pid}-39/alias':root/f'durable/tmp/path-fixture-{pid}-39',
            f'durable/tmp/ancestor-initial-directory-{pid}-8/parent':root/f'durable/tmp/ancestor-initial-directory-{pid}-8/outside',
            f'durable/tmp/checkpoint-{pid}-40/store/checkpoint-1.rec':root/f'durable/tmp/path-fixture-{pid}-39/own-sentinel',
            f'durable/tmp/journal-{pid}-41/store/journal-1.rec':root/f'durable/tmp/path-fixture-{pid}-39/own-sentinel',
            f'runtime/tmp/runtime-{pid}-3/project/main.tex':root/f'runtime/tmp/runtime-{pid}-3/own-outside-sentinel'}
        for name,target in targets.items():
            path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.symlink_to(target)
        normal={'name':'normal-0','own_pid':pid};(root/'normal-0.json').write_text(json.dumps(normal))
        return a,root,[normal],targets

    def test_B21_source_defined_negative_links_are_custodied_without_following(self):
        a,root,normals,targets=self.actual_link_fixture();q=a._actual_fixture_inventory(root,normals)
        self.assertEqual(len(q['normal_negative_symlinks']),5)
        self.assertEqual(set(q['normal_negative_symlinks']),set(targets))
        self.assertEqual(set(q['files']),{'normal-0.json'})
        with self.assertRaises(ValueError):a._fixture_inventory(root)

    def test_B22_other_process_negative_links_are_not_normal_owned(self):
        a,root,normals,_=self.actual_link_fixture();normals[0]['own_pid']+=1
        (root/'normal-0.json').write_text(json.dumps(normals[0]))
        with self.assertRaises(ValueError):a._actual_fixture_inventory(root,normals)

    def test_B23_known_negative_link_name_requires_its_exact_source_target(self):
        a,root,normals,targets=self.actual_link_fixture();path=root/next(iter(targets));path.unlink();path.symlink_to(root/'wrong-target')
        with self.assertRaises(ValueError):a._actual_fixture_inventory(root,normals)

    def test_B24_normal_negative_link_policy_never_admits_parent_fixture_links(self):
        a,root,normals,_=self.actual_link_fixture();path=root/'durable/tmp/child-append-JournalWrite/alias'
        path.parent.mkdir();path.symlink_to(root)
        with self.assertRaises(ValueError):a._actual_fixture_inventory(root,normals)

    def test_B25_normal_negative_link_PID_is_never_boolean_evidence(self):
        a,root,normals,_=self.actual_link_fixture();normals[0]['own_pid']=True
        (root/'normal-0.json').write_text(json.dumps(normals[0]))
        with self.assertRaises(ValueError):a._actual_fixture_inventory(root,normals)

    def test_B26_missing_normal_negative_link_is_not_silent_partial_custody(self):
        a,root,normals,targets=self.actual_link_fixture();(root/next(iter(targets))).unlink()
        with self.assertRaises(ValueError):a._actual_fixture_inventory(root,normals)


if __name__=='__main__':unittest.main()
