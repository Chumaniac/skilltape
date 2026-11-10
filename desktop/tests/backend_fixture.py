"""Synthetic loopback child; output is confined to its private test directory."""
import http.server
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile

port, requested_output, mode = int(sys.argv[1]), sys.argv[2], sys.argv[3]
if not 1 <= port <= 65535 or mode not in {"normal", "hang", "managed"}:
    raise SystemExit("FIXTURE_ARGUMENT_INVALID")
# The caller creates this private UUID directory. Never accept an arbitrary path.
directory_name = os.path.basename(os.path.dirname(requested_output))
filename = os.path.basename(requested_output)
uuid = r"[A-Fa-f0-9]{8}(?:-[A-Fa-f0-9]{4}){3}-[A-Fa-f0-9]{12}"
if not re.fullmatch("backend-check-" + uuid, directory_name) or not re.fullmatch(uuid + r"\.json", filename):
    raise SystemExit("FIXTURE_OUTPUT_INVALID")
root = Path(tempfile.gettempdir()).resolve() / directory_name
info = root.lstat()
if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
        or stat.S_IMODE(info.st_mode) != 0o700):
    raise SystemExit("FIXTURE_DIRECTORY_INVALID")
output = root / filename
if Path(requested_output).resolve() != output:
    raise SystemExit("FIXTURE_OUTPUT_INVALID")


def write_result(path, content):
    # Exclusive creation and NOFOLLOW protect existing files and links.
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "w") as stream:
        stream.write(content)


if mode == "hang":
    signal.signal(signal.SIGTERM, signal.SIG_IGN)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "/target")
            self.end_headers()
            return
        if self.path == "/target":
            try:
                write_result(output.with_suffix(".redirected"), "unexpected")
            except FileExistsError:
                pass
        body = b'{"service":"fixture-ready"}'
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
write_result(output, json.dumps({"pid": os.getpid(), "cwd": os.getcwd()}))
if mode == "managed":
    state = output.with_suffix(".state.json")
    write_result(state, json.dumps({"pid": os.getpid(), "port": port, "started": subprocess.check_output(
        ["ps", "-p", str(os.getpid()), "-o", "lstart="], text=True).strip()}))
server.serve_forever()
