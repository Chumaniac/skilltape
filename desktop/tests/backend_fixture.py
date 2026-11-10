"""Synthetic loopback child for native lifecycle checks; no project/user files."""
import http.server
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

port, output, mode = int(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
if mode == 'hang':
    signal.signal(signal.SIGTERM, signal.SIG_IGN)

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/redirect':
            self.send_response(302)
            self.send_header('Location', '/target')
            self.end_headers()
            return
        if self.path == '/target':
            output.with_suffix('.redirected').write_text('unexpected')
        body = b'{"service":"fixture-ready"}'
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args):
        pass

server = http.server.ThreadingHTTPServer(('127.0.0.1', port), Handler)
output.write_text(json.dumps({'pid': os.getpid(), 'cwd': os.getcwd()}))
if mode == 'managed':
    state = output.with_suffix('.state.json')
    state.write_text(json.dumps({'pid': os.getpid(), 'port': port, 'started': subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).strip()}))
    state.chmod(0o600)
server.serve_forever()
