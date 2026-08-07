#!/usr/bin/env bash
set -Eeuo pipefail

script_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temp_root="$(mktemp -d "${TMPDIR:-/tmp}/skilltape-install-test.XXXXXX")"
server_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$temp_root"
}
trap cleanup EXIT

target="x86_64-apple-darwin"
version="0.1.0"
binary_dir="$temp_root/target/$target/release"
ui_dist="$temp_root/ui-dist"
release_dir="$temp_root/releases/download/v$version"
install_root="$temp_root/install"
install_dir="$install_root/bin"
mkdir -p "$binary_dir" "$ui_dist/assets" "$release_dir" "$install_dir"
printf 'new cli\n' > "$binary_dir/skilltape"
printf 'new api\n' > "$binary_dir/skilltape-console-api"
printf '<main>Console</main>\n' > "$ui_dist/index.html"
printf 'console.log("ok")\n' > "$ui_dist/assets/app.js"
python3 "$script_root/scripts/package_release.py" \
  --version "$version" \
  --target "$target" \
  --binary-dir "$binary_dir" \
  --ui-dist "$ui_dist" \
  --output-dir "$release_dir" >/dev/null

archive="$release_dir/skilltape-v$version-$target.tar.gz"
if command -v sha256sum >/dev/null 2>&1; then
  digest_command=(sha256sum)
else
  digest_command=(shasum -a 256)
fi
"${digest_command[@]}" "$archive" | awk -v asset="$(basename "$archive")" '{print $1 "  " asset}' > "$release_dir/checksums.txt"
printf 'old cli\n' > "$install_dir/skilltape"

port="$(python3 - <<'PY'
import socket

with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
certificate="$temp_root/server.crt"
private_key="$temp_root/server.key"
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "$private_key" -out "$certificate" -days 1 \
  -subj '/CN=127.0.0.1' \
  -addext 'subjectAltName=IP:127.0.0.1' >/dev/null 2>&1
python3 - "$port" "$temp_root" "$certificate" "$private_key" <<'PY' \
  >"$temp_root/https.log" 2>&1 &
import http.server
import ssl
import sys

port, root, certificate, private_key = sys.argv[1:]
handler = lambda *args, **kwargs: http.server.SimpleHTTPRequestHandler(
    *args, directory=root, **kwargs
)
server = http.server.ThreadingHTTPServer(("127.0.0.1", int(port)), handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(certificate, private_key)
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
PY
server_pid=$!
for _ in {1..50}; do
  if python3 - "$port" <<'PY'
import socket
import sys

with socket.socket() as sock:
    sock.settimeout(0.1)
    try:
        sock.connect(("127.0.0.1", int(sys.argv[1])))
    except OSError:
        raise SystemExit(1)
PY
  then
    break
  fi
  sleep 0.1
done

CURL_CA_BUNDLE="$certificate" \
SKILLTAPE_RELEASE_BASE_URL="https://127.0.0.1:$port/releases/download" \
SKILLTAPE_VERSION="$version" \
SKILLTAPE_INSTALL_DIR="$install_dir" \
SKILLTAPE_TARGET="$target" \
bash "$script_root/scripts/install.sh" >/dev/null

grep -Fq 'new cli' "$install_dir/skilltape"
grep -Fq 'new api' "$install_dir/skilltape-console-api"
grep -Fq '<main>Console</main>' "$install_root/console/index.html"

failure_root="$temp_root/failure-install"
failure_dir="$failure_root/bin"
mkdir -p "$failure_dir"
printf 'preserved cli\n' > "$failure_dir/skilltape"
printf '%064d  %s\n' 0 "$(basename "$archive")" > "$release_dir/checksums.txt"
if CURL_CA_BUNDLE="$certificate" \
  SKILLTAPE_RELEASE_BASE_URL="https://127.0.0.1:$port/releases/download" \
  SKILLTAPE_VERSION="$version" \
  SKILLTAPE_INSTALL_DIR="$failure_dir" \
  SKILLTAPE_TARGET="$target" \
  bash "$script_root/scripts/install.sh" >/dev/null 2>&1; then
  echo "checksum failure unexpectedly succeeded" >&2
  exit 1
fi
grep -Fq 'preserved cli' "$failure_dir/skilltape"
[[ ! -e "$failure_root/console/index.html" ]]

echo "installer fixture passed"
