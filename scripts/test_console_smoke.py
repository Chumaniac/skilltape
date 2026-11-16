"""The installed smoke must recognize the actual localized UI and reject wrong products."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = r'''#!/usr/bin/env python3
import json,os,signal,socket
from http.server import BaseHTTPRequestHandler,HTTPServer
case=os.environ.get('SMOKE_CASE','valid')
if case=='ignore-term':signal.signal(signal.SIGTERM,signal.SIG_IGN)
if case=='no-dns':
 def forbidden_dns(*args):raise RuntimeError('local fixture must not resolve a hostname')
 socket.getfqdn=forbidden_dns
identity={'schema':'skilltape.build-info/v1','version':'0.5.1','sourceRevision':'a'*40,'dirty':False,'channel':'source','capabilities':['readonly-console/v1']}
class Handler(BaseHTTPRequestHandler):
 def log_message(self,*args):pass
 def do_GET(self):
  value=None
  if self.path=='/api/v1/workspaces':value={'schema':'skilltape.dev/console/v1','items':[{'id':'synthetic'}]}
  if self.path=='/api/v1/build-info':value=identity
  if self.path=='/build-info.json':value={**identity,'version':'0.4.1'} if case=='mixed' else identity
  if value is not None:body=json.dumps(value).encode()
  elif self.path=='/':body=('<!doctype html><!-- SkillTape Console --><title>'+('Other Product' if case=='wrong' else 'SkillTape 控制台')+'</title><script type="module" src="/assets/app.js"></script><link rel="stylesheet" href="/assets/app.css">').encode()
  elif self.path in ['/assets/app.js','/assets/app.css']:body=b'owned synthetic asset'
  else:self.send_response(404);self.end_headers();return
  self.send_response(200);self.end_headers();self.wfile.write(body)
class LocalServer(HTTPServer):
 def server_bind(self):
  # The real Rust service binds numerically; the fixture must not depend on DNS.
  self.socket.bind(self.server_address)
  self.server_address=self.socket.getsockname()
  self.server_name='127.0.0.1';self.server_port=self.server_address[1]
server=LocalServer(('127.0.0.1',0),Handler)
print('SkillTape Console API listening at http://127.0.0.1:'+str(server.server_port),flush=True)
server.serve_forever()
'''

class ConsoleSmokeTests(unittest.TestCase):
    def run_case(self, case):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);(root/'workspace').mkdir();(root/'ui').mkdir();(root/'ui/index.html').write_text('fixture UI')
            executable=root/'owned-api';executable.write_text(FIXTURE.replace('#!/usr/bin/env python3', '#!'+sys.executable));executable.chmod(0o700)
            return subprocess.run(['bash',str(ROOT/'scripts/smoke_console.sh'),str(root/'workspace')],env={**os.environ,'SMOKE_CASE':case,'SKILLTAPE_CONSOLE_API_BIN':str(executable),'SKILLTAPE_CONSOLE_UI_DIST':str(root/'ui')},capture_output=True,text=True,timeout=10)
    def test_wrong_product_at_http_200_cannot_pass_on_a_comment(self):
        result=self.run_case('wrong');self.assertNotEqual(result.returncode,0)
    def test_actual_localized_product_and_matching_identity_pass(self):
        result=self.run_case('valid');self.assertEqual(result.returncode,0,result.stderr)
    def test_mixed_component_versions_fail(self):
        result=self.run_case('mixed');self.assertNotEqual(result.returncode,0)
    def test_loopback_fixture_does_not_require_reverse_dns(self):
        result=self.run_case('no-dns');self.assertEqual(result.returncode,0,result.stderr)
    def test_a_stubborn_owned_child_is_reaped_within_the_original_timeout(self):
        result=self.run_case('ignore-term');self.assertEqual(result.returncode,0,result.stderr)

if __name__=='__main__':unittest.main()
