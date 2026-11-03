"""Same-host public C ABI + authenticated Unix IPC controls. No live app.

This parent never kills a process. Each C helper installs its own 9s alarm and
required sandbox before opening the one real production Runtime. All handles,
descriptor paths and cleanup actions are retained in this parent's receipt.
"""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import selectors
import socket
import stat
import struct
import subprocess
import sys
import time
import traceback

OUT = Path(sys.argv[1]).resolve()
LABEL = sys.argv[2]
INPUT = json.loads((OUT / 'receipts/input.json').read_text())
NONCE = INPUT['nonce']
RUN = OUT / 'fixtures' / LABEL
RUN.mkdir(mode=0o700)
EVENTS, CHILDREN, RESULTS = [], [], []
CASE = None


def digest(data):
    return hashlib.sha256(data).hexdigest()


def record(**data):
    EVENTS.append({'case': CASE, 't': time.monotonic(), **data})


def good(reply):
    assert reply.get('ok') is True, reply
    return reply['result']


def code(reply, expected):
    assert reply.get('ok') is False and reply['error']['code'] == expected, reply


def read_line(stream, timeout=3.5):
    # Read exactly a line without a buffered-reader/select race.
    end = time.monotonic() + timeout
    raw = bytearray()
    with selectors.DefaultSelector() as sel:
        sel.register(stream, selectors.EVENT_READ)
        while time.monotonic() < end:
            assert sel.select(max(0, end-time.monotonic())), 'helper read deadline'
            b = os.read(stream.fileno(), 1)
            if not b:
                raise EOFError('helper ended before reply')
            raw.extend(b)
            assert len(raw) <= 1024*1024+1
            if b == b'\n':
                return json.loads(raw)
    raise TimeoutError('helper reply deadline')


class Host:
    def __init__(self, project, kind='dynamic', rejected=False):
        self.project = project
        self.path = None
        self.descriptor = None
        self.closed = False
        self.start = time.monotonic()
        binary = OUT / 'binaries' / ('cabi-' + kind)
        self.p = subprocess.Popen([str(binary), str(project), str(OUT)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, bufsize=0)
        self.meta = {'pid': self.p.pid, 'case': CASE, 'kind': kind,
                     'helper': str(binary), 'helper_sha256': digest(binary.read_bytes()),
                     'project': str(project), 'nonce': NONCE,
                     'argv': [str(binary), str(project), str(OUT)]}
        CHILDREN.append(self)
        first = read_line(self.p.stdout)
        record(layer='C_ABI', event='factory', pid=self.p.pid, reply=first)
        if rejected:
            assert first == {'factory_rejected': True}, first
            self.finish(65)
            return
        self.info = good(first)
        assert self.info['recovery']['enabled'] is True
        self.path = Path(self.info['session_descriptor'])
        m = self.path.lstat()
        raw = self.path.read_bytes()
        self.descriptor = json.loads(raw)
        d = self.descriptor
        assert d['project_root'] == str(project.resolve()) and d['version'] == 1
        assert len(d['token']) == 64 and all(c in '0123456789abcdef' for c in d['token'])
        assert self.path.name.startswith(str(self.p.pid) + '-')
        assert m.st_uid == os.geteuid() and stat.S_IMODE(m.st_mode) == 0o600
        dm = self.path.parent.stat()
        assert dm.st_uid == os.geteuid() and stat.S_IMODE(dm.st_mode) == 0o700
        self.meta.update(descriptor_path=str(self.path), descriptor_sha256=digest(raw),
                         endpoint=d['endpoint'], session_id=d['session_id'])

    def native(self, **request):
        self.p.stdin.write(json.dumps(request, ensure_ascii=True).encode() + b'\n')
        reply = read_line(self.p.stdout)
        record(layer='C_ABI', pid=self.p.pid, request=request, reply=reply)
        return reply

    def connect(self):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(3.5)
        s.connect(self.descriptor['endpoint'])
        return s

    def envelope(self, request):
        return json.dumps({'version': 1, 'token': self.descriptor['token'],
                           'request': request}, ensure_ascii=True).encode() + b'\n'

    def ipc(self, **request):
        with self.connect() as s:
            s.sendall(self.envelope(request))
            raw = bytearray()
            while not raw.endswith(b'\n'):
                b = s.recv(8192)
                assert b and len(raw) <= 1024*1024
                raw.extend(b)
        reply = json.loads(raw)
        record(layer='authenticated_IPC', pid=self.p.pid, request=request, reply=reply)
        return reply

    def finish(self, expected=0):
        if self.closed:
            return
        if self.p.stdin and not self.p.stdin.closed:
            self.p.stdin.close()
        # It owns a self-exit deadline even if StopJoin has a bug. No parent kill.
        remaining = max(0.01, 10-(time.monotonic()-self.start))
        rc = self.p.wait(timeout=remaining)
        stderr = self.p.stderr.read().decode(errors='replace')
        self.meta.update(exit_code=rc, duration_seconds=time.monotonic()-self.start,
                         stderr=stderr, waited_own_child=True, expected_exit=expected)
        self.closed = True
        assert 'SANDBOX_ACTIVE' in stderr, stderr
        assert rc == expected, self.meta
        if expected == 0:
            assert 'TEARDOWN_COMPLETE' in stderr
            assert not self.path.exists(), 'normal teardown retained descriptor'
        cleanup = []
        # Only exact own retained paths, after wait. Never list shared sessions.
        if self.descriptor:
            for p in [self.path, Path(self.descriptor['endpoint'])]:
                if p.exists():
                    m = p.lstat()
                    assert m.st_uid == os.geteuid() and p.name.startswith(str(self.p.pid)+'-')
                    p.unlink()
                    cleanup.append(str(p))
        self.meta['own_cleanup_after_wait'] = cleanup
        self.p.stdout.close()
        self.p.stderr.close()


def fixture(name, raw=b'abc'):
    p = RUN / name
    p.mkdir(mode=0o700)
    (p / 'main.tex').write_bytes(raw)
    (p / 'fixture-nonce.txt').write_text(NONCE)
    return p


def edit(rev, end, text):
    return {'op': 'apply_edits', 'document_id': 'main.tex', 'expected_revision': rev,
            'edits': [{'start': {'line': 0, 'character': 0},
                       'end': {'line': 0, 'character': end}, 'text': text}]}


def opened(h):
    return good(h.native(op='open_document', path='main.tex'))['revision']


def text(h, expected, revision, dirty=None, layer='ipc'):
    call = h.ipc if layer == 'ipc' else h.native
    d = good(call(op='read_document', document_id='main.tex', revision=revision))
    assert d['text'] == expected and d['revision'] == revision, d
    if dirty is not None:
        assert d['dirty'] is dirty, d
    return d


def lifecycle(kind):
    p = fixture('lifecycle-'+kind)
    h = Host(p, kind)
    r = opened(h)
    n = good(h.native(**edit(r, 3, 'N😀')))
    assert n['last_origin'] == 'user'
    a = good(h.ipc(**edit(n['revision'], 3, 'A\ufeff😀')))
    assert a['last_origin'] == 'agent'
    assert (p/'main.tex').read_bytes() == b'abc'
    assert good(h.native(op='workspace_info'))['session_descriptor'] == h.info['session_descriptor']
    saved = good(h.ipc(op='save_document', document_id='main.tex', expected_revision=a['revision']))
    assert saved['dirty'] is False and (p/'main.tex').read_text() == 'A\ufeff😀'
    u = good(h.native(op='undo')); text(h, 'N😀', u['revision'], True)
    r = good(h.ipc(op='redo')); text(h, 'A\ufeff😀', r['revision'], False, 'native')
    ack = text(h, 'A\ufeff😀', r['revision'], False)
    info = good(h.ipc(op='workspace_info')); h.finish()
    h2 = Host(p, kind)
    assert h2.info['recovery']['restored'] is True
    assert h2.info['recovery']['sequence'] == info['recovery']['sequence']
    assert text(h2, ack['text'], ack['revision'], False, 'native') == ack
    code(h2.ipc(**edit(a['revision'], 4, 'stale')), 'revision_conflict')
    u = good(h2.ipc(op='undo')); text(h2, 'N😀', u['revision'], True)
    assert (p/'main.tex').read_text() == 'A\ufeff😀'
    r = good(h2.native(op='redo')); text(h2, 'A\ufeff😀', r['revision'], False)
    h2.finish()


def concurrent_native_agent():
    p = fixture('concurrent'); h = Host(p); r = opened(h)
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        fs = [pool.submit(h.native, **edit(r, 3, 'native')),
              pool.submit(h.ipc, **edit(r, 3, 'agent'))]
        replies = [f.result(timeout=4) for f in fs]
    winners = [x for x in replies if x['ok']]
    assert len(winners) == 1
    code(next(x for x in replies if not x['ok']), 'revision_conflict')
    win = winners[0]['result']; expected = 'native' if replies[0]['ok'] else 'agent'
    text(h, expected, r+1, True)
    assert win['last_origin'] == ('user' if replies[0]['ok'] else 'agent')
    seq = good(h.native(op='workspace_info'))['recovery']['sequence']
    assert good(h.ipc(op='workspace_info'))['recovery']['sequence'] == seq
    h.finish(); h = Host(p); text(h, expected, r+1, True); h.finish()


def unicode_composition():
    p = fixture('unicode', b'\xef\xbb\xbfA\xf0\x9f\x98\x80\r\n')
    h = Host(p); r = opened(h)
    d = text(h, 'A😀\n', r, False)
    assert d['bom'] is True and d['line_ending'] == 'crlf'
    code(h.ipc(**edit(r, 2, 'bad')), 'invalid_position')
    good(h.native(op='begin_composition', document_id='main.tex', expected_revision=r))
    code(h.ipc(**edit(r, 3, 'blocked')), 'composition_active')
    req = edit(r, 3, '\ufeff😀'); req['op'] = 'end_composition'
    v = good(h.native(**req)); text(h, '\ufeff😀\n', v['revision'], True)
    good(h.ipc(op='save_document', document_id='main.tex', expected_revision=v['revision']))
    assert (p/'main.tex').read_bytes() == b'\xef\xbb\xbf\xef\xbb\xbf\xf0\x9f\x98\x80\r\n'
    h.finish(); h = Host(p); text(h, '\ufeff😀\n', v['revision'], False); h.finish()


def external_replacement():
    for name, replacement in [('inode', b'abc'), ('content', b'outside')]:
        p = fixture('external-'+name); h = Host(p); r = opened(h)
        r = good(h.ipc(**edit(r, 3, 'dirty')))['revision']
        h.finish(); tmp = p/'replacement.tex'; tmp.write_bytes(replacement)
        tmp.replace(p/'main.tex'); h = Host(p)
        code(h.native(op='save_document', document_id='main.tex', expected_revision=r), 'external_change')
        assert (p/'main.tex').read_bytes() == replacement
        text(h, 'dirty', r, True); h.finish()


def malformed_auth_and_ack():
    p = fixture('auth'); h = Host(p); r = opened(h)
    seq = good(h.native(op='workspace_info'))['recovery']['sequence']
    base = {'version': 1, 'token': h.descriptor['token'], 'request': edit(r, 3, 'MUST_NOT_COMMIT')}
    wrong = [({**base, 'token': '0'*64}, 'unauthorized'),
             ({**base, 'version': 2}, 'protocol_mismatch'),
             ({'version': 1, 'request': base['request']}, 'invalid_request'),
             ({**base, 'request': {'op': 'ack', 'sequence': seq}}, 'invalid_request')]
    for envelope, expected in wrong:
        with h.connect() as s:
            s.sendall(json.dumps(envelope).encode()+b'\n')
            reply = json.loads(s.makefile('rb').readline())
        code(reply, expected); record(layer='IPC_negative', expected=expected, reply=reply)
    # Duplicate ACK field and missing/duplicate request fields reject deserialization.
    for raw in [b'{', b'{"version":1,"version":1}',
                b'{"version":1,"token":'+json.dumps(h.descriptor['token']).encode()+b',"request":{"op":"ack","sequence":1,"sequence":1}}']:
        with h.connect() as s:
            s.sendall(raw+b'\n'); reply = json.loads(s.makefile('rb').readline())
        code(reply, 'invalid_request'); record(layer='IPC_negative', control='malformed_or_duplicate_ack', reply=reply)
    text(h, 'abc', r, False)
    assert good(h.ipc(op='workspace_info'))['recovery']['sequence'] == seq
    h.finish()


def partial_request():
    p = fixture('partial-request'); h = Host(p); r = opened(h)
    raw = h.envelope(edit(r, 3, 'UNDELIVERED'))
    with h.connect() as s:
        s.sendall(raw[:len(raw)//2]); s.shutdown(socket.SHUT_WR)
        reply = json.loads(s.makefile('rb').readline())
    code(reply, 'invalid_request'); record(layer='IPC', control='partial_request', reply=reply)
    text(h, 'abc', r, False); h.finish()
    h = Host(p); text(h, 'abc', r, False); h.finish()


def incomplete_request_teardown():
    p = fixture('incomplete-teardown'); h = Host(p); r = opened(h)
    s = h.connect()
    s.sendall(h.envelope(edit(r, 3, 'never-complete'))[:20])
    # Do not finish the frame or half-close: real IPC read deadline must bound
    # StopJoin. The parent's stdin EOF invokes public runtime_free in C.
    start = time.monotonic()
    h.finish()
    elapsed = time.monotonic()-start
    s.close()
    assert elapsed < 5, elapsed
    record(layer='C_ABI_and_IPC', control='incomplete_frame_stop_join', seconds=elapsed)
    h = Host(p); text(h, 'abc', r, False); h.finish()


def ipc_acknowledged_then_process_exit():
    p = fixture('ipc-ack-process-exit'); h = Host(p); r = opened(h)
    r = good(h.ipc(**edit(r, 3, 'agent-ack')))['revision']
    good(h.native(op='save_document', document_id='main.tex', expected_revision=r))
    r = good(h.ipc(op='undo'))['revision']
    ack = text(h, 'abc', r, True)
    sequence = good(h.ipc(op='workspace_info'))['recovery']['sequence']
    h.p.stdin.write(b'!exit\n'); h.finish(73)
    h = Host(p)
    assert h.info['recovery']['sequence'] == sequence
    assert text(h, 'abc', ack['revision'], True, 'native') == ack
    r = good(h.ipc(op='redo'))['revision']
    text(h, 'agent-ack', r, False)
    assert (p/'main.tex').read_text() == 'agent-ack'
    h.finish()


def disconnected_request():
    p = fixture('disconnect'); h = Host(p); r = opened(h)
    with h.connect() as s:
        s.sendall(h.envelope(edit(r, 3, 'committed-without-observed-reply')))
        s.shutdown(socket.SHUT_RDWR)
    record(layer='IPC', control='full_request_disconnected', client_reply_observed=False, replay=False)
    # A new connection is serialized after the first dispatch; never resend mutation.
    ack = text(h, 'committed-without-observed-reply', r+1, True)
    seq = good(h.ipc(op='workspace_info'))['recovery']['sequence']
    h.finish(); h = Host(p)
    assert h.info['recovery']['sequence'] == seq
    text(h, ack['text'], ack['revision'], True)
    good(h.native(op='undo')); text(h, 'abc', r+2, False); h.finish()


def partial_reply():
    p = fixture('partial-reply'); h = Host(p); r = opened(h)
    with h.connect() as s:
        s.sendall(h.envelope(edit(r, 3, 'partial-reply')))
        prefix = s.recv(1)
        assert prefix == b'{'
    record(layer='IPC', control='one_reply_byte_then_disconnect', observed_bytes=1,
           full_client_reply_observed=False, replay=False)
    text(h, 'partial-reply', r+1, True); h.finish()
    h = Host(p); text(h, 'partial-reply', r+1, True); h.finish()


def dropped_cabi_reply():
    p = fixture('cabi-dropped'); h = Host(p); r = opened(h)
    h.p.stdin.write(b'!drop_reply\n'+json.dumps(edit(r, 3, 'C-result-not-delivered')).encode()+b'\n')
    h.finish(73)
    record(layer='C_ABI', control='real_result_freed_without_parent_delivery', replay=False)
    h = Host(p); text(h, 'C-result-not-delivered', r+1, True)
    good(h.ipc(op='undo')); text(h, 'abc', r+2, False); h.finish()


def selector(root):
    p = root/'.latexide/recovery/current'; raw = p.read_bytes()
    assert raw[:8] == b'LXPTR001' and digest(raw[:-64]).encode() == raw[-64:]
    payload = raw[24:-64]; seq = struct.unpack('<Q', payload[:8])[0]
    at, fields = 8, []
    for _ in range(4):
        n = struct.unpack('<Q', payload[at:at+8])[0]; at += 8
        fields.append(payload[at:at+n]); at += n
    assert at == len(payload)
    return p, seq, fields


def rewrite_selector(p, seq, fields):
    payload = struct.pack('<Q', seq) + b''.join(struct.pack('<Q', len(f))+f for f in fields)
    header = b'LXPTR001'+struct.pack('<QQ', 1, len(payload))
    raw = header+payload; p.write_bytes(raw+digest(raw).encode())


def damaged_store(kind):
    p = fixture('damage-'+kind); h = Host(p); r = opened(h)
    r = good(h.ipc(**edit(r, 3, 'acknowledged')))['revision']
    h.finish(); sel, seq, fields = selector(p)
    cohort = sel.parent/fields[0].decode()
    if kind == 'journal-tail':
        target = cohort/'journal-1.rec'; target.write_bytes(target.read_bytes()+b'LXRECOV\x00torn')
    elif kind == 'journal-cut':
        target = cohort/'journal-1.rec'; target.write_bytes(target.read_bytes()[:-1])
    elif kind == 'selector-cut':
        target = sel; target.write_bytes(target.read_bytes()[:-9])
    else:
        target = sel
        fields[3] = {'missing-ack': b'', 'wrong-ack': b'invalid-receipt',
                     'double-ack': fields[3]+fields[3]}[kind]
        rewrite_selector(sel, seq, fields)
    bad = target.read_bytes(); Host(p, rejected=True)
    assert target.read_bytes() == bad and (p/'main.tex').read_bytes() == b'abc'
    record(layer='C_ABI_factory', control=kind, corrupted_path=str(target),
           corrupted_sha256=digest(bad), no_fallback=True)


def owner_lock_and_modes():
    p = fixture('owner-lock'); h = Host(p); r = opened(h)
    Host(p, rejected=True)
    text(h, 'abc', r, False)
    store = p/'.latexide/recovery'
    for f in [store, *store.rglob('*')]:
        m = f.lstat(); assert m.st_uid == os.geteuid() and not f.is_symlink()
        assert stat.S_IMODE(m.st_mode) == (0o700 if f.is_dir() else 0o600), str(f)
        if f.is_file(): assert m.st_nlink == 1
    h.finish(); h = Host(p); text(h, 'abc', r, False); h.finish()
    os.chmod(store, 0o755); Host(p, rejected=True)
    assert stat.S_IMODE(store.stat().st_mode) == 0o755
    os.chmod(store, 0o700)


CONTROLS = [('dynamic_save_restart_history', lambda: lifecycle('dynamic')),
            ('static_save_restart_history', lambda: lifecycle('static')),
            ('concurrent_native_agent', concurrent_native_agent),
            ('unicode_composition', unicode_composition),
            ('external_replacement', external_replacement),
            ('malformed_auth_ack', malformed_auth_and_ack),
            ('partial_request', partial_request),
            ('incomplete_request_teardown', incomplete_request_teardown),
            ('ipc_acknowledged_process_exit', ipc_acknowledged_then_process_exit),
            ('disconnected_request', disconnected_request),
            ('partial_reply', partial_reply),
            ('dropped_cabi_reply', dropped_cabi_reply),
            *[('damaged_'+k, lambda k=k: damaged_store(k)) for k in
              ['journal-tail', 'journal-cut', 'selector-cut', 'missing-ack', 'wrong-ack', 'double-ack']],
            ('owner_lock_modes_teardown', owner_lock_and_modes)]


def main():
    global CASE
    for name, fn in CONTROLS:
        CASE = name; start = time.monotonic()
        try:
            fn(); result = {'id': name, 'state': 'pass'}
        except Exception as e:
            result = {'id': name, 'state': 'fail' if isinstance(e, AssertionError) else 'error',
                      'detail': str(e), 'traceback': traceback.format_exc()}
        for h in CHILDREN:
            if not h.closed:
                try: h.finish()
                except Exception as e:
                    result['cleanup_error'] = repr(e)
                    if result['state'] == 'pass': result['state'] = 'error'
        result['duration_seconds'] = time.monotonic()-start
        RESULTS.append(result); print(json.dumps(result), flush=True)
        if any('SANDBOX_BLOCKED' in h.meta.get('stderr', '') for h in CHILDREN):
            break  # Fail closed, never try an unsandboxed helper.
    fs = {str(p.relative_to(RUN)): {'sha256': digest(p.read_bytes()), 'size': p.stat().st_size,
          'mode': stat.S_IMODE(p.stat().st_mode)} for p in RUN.rglob('*') if p.is_file()}
    rec = {'schema': 1, 'nonce': NONCE, 'label': LABEL, 'results': RESULTS,
           'counts': {s: sum(r['state'] == s for r in RESULTS) for s in ['pass','fail','error']},
           'unrun': [n for n,_ in CONTROLS if n not in {r['id'] for r in RESULTS}],
           'events': EVENTS, 'children': [h.meta for h in CHILDREN],
           'fixture_root': str(RUN), 'fixture_files': fs,
           'auth_protocol_modified': False, 'mutation_auto_replay': False,
           'client_ack_exactly_once': 'blocked: v1 has no client ACK/deduplication contract'}
    (OUT/'receipts'/(LABEL+'-cases.json')).write_text(json.dumps(rec, indent=2)+'\n')
    return 0 if len(RESULTS) == len(CONTROLS) and all(r['state'] == 'pass' for r in RESULTS) else 1


if __name__ == '__main__':
    sys.exit(main())
