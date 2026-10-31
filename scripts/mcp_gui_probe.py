"""Apply/save one reversible comment via MCP to the explicitly selected GUI test project."""
import argparse,json,selectors,subprocess
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--session',type=Path,required=True);args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
process=subprocess.Popen([str(root/'dist/LaTeXIDE.app/Contents/Resources/latexide-mcp'),'--session',str(args.session)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0)
sequence=0
try:
 def rpc(method,params):
  global sequence
  sequence+=1;process.stdin.write(json.dumps(dict(jsonrpc='2.0',id=sequence,method=method,params=params)).encode()+b'\n')
  data=bytearray()
  with selectors.DefaultSelector() as selector:
   selector.register(process.stdout,selectors.EVENT_READ)
   while b'\n' not in data:
    if not selector.select(8):raise RuntimeError('MCP timeout')
    chunk=process.stdout.read(1)
    if not chunk:raise RuntimeError('MCP EOF')
    data.extend(chunk)
  reply=json.loads(data)
  if 'error' in reply:raise RuntimeError(reply['error'])
  return reply['result']
 def tool(name,arguments):
  result=json.loads(rpc('tools/call',dict(name=name,arguments=arguments))['content'][0]['text'])
  if not result['ok']:raise RuntimeError(result['error'])
  return result['result']
 rpc('initialize',dict(protocolVersion='2025-11-25',capabilities={},clientInfo=dict(name='latexide-gui-probe',version='1')))
 process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
 document=tool('read_document',dict(document_id='main.tex'))
 changed=tool('apply_edits',dict(document_id='main.tex',expected_revision=document['revision'],edits=[dict(start=dict(line=0,character=0),end=dict(line=0,character=0),text='% MCP GUI edit: Árvíz 😀\n')]))
 saved=tool('save_document',dict(document_id='main.tex',expected_revision=changed['revision']))
 print(json.dumps(dict(mcp_initialized=True,edit_applied=True,saved=True,revision=saved['revision']),ensure_ascii=False))
finally:
 process.stdin.close()
 try:process.wait(timeout=5)
 except subprocess.TimeoutExpired:process.kill();process.wait()
 process.stdout.close();process.stderr.close()
