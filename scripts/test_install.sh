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
"${digest_command[@]}" "$archive" | awk -v asset="./$(basename "$archive")" '{print $1 "  " asset}' > "$release_dir/checksums.txt"
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

# Inject one final-move failure through a fixture-only command wrapper. The
# production installer has no test bypass or failure-injection environment.
transaction_root="$temp_root/transaction-install"
transaction_dir="$transaction_root/bin"
mkdir -p "$transaction_dir" "$transaction_root/console/assets" "$temp_root/mv-wrapper"
mkdir -p "$transaction_root/data"
printf 'preserved receipt\n' > "$transaction_root/data/receipt.json"
printf 'transaction old cli\n' > "$transaction_dir/skilltape"
printf 'transaction old api\n' > "$transaction_dir/skilltape-console-api"
printf 'transaction old UI\n' > "$transaction_root/console/index.html"
printf 'transaction old asset\n' > "$transaction_root/console/assets/old.js"
chmod 0700 "$transaction_dir/skilltape" "$transaction_dir/skilltape-console-api"
cat > "$temp_root/mv-wrapper/mv" <<'SH'
#!/usr/bin/env bash
set -eu
for argument in "$@"; do destination="$argument"; done
if [[ "$destination" == "$SKILLTAPE_FIXTURE_FAIL_DEST" ]] && \
   [[ ! -e "$SKILLTAPE_FIXTURE_ONCE" || "${SKILLTAPE_FIXTURE_ALWAYS:-0}" == 1 ]]; then
  : > "$SKILLTAPE_FIXTURE_ONCE"
  exit 17
fi
exec /bin/mv "$@"
SH
chmod 0755 "$temp_root/mv-wrapper/mv"
if PATH="$temp_root/mv-wrapper:$PATH" CURL_CA_BUNDLE="$certificate" \
  SKILLTAPE_FIXTURE_FAIL_DEST="$transaction_root/console" \
  SKILLTAPE_FIXTURE_ONCE="$temp_root/move-failed-once" \
  SKILLTAPE_RELEASE_BASE_URL="https://127.0.0.1:$port/releases/download" \
  SKILLTAPE_VERSION="$version" SKILLTAPE_INSTALL_DIR="$transaction_dir" SKILLTAPE_TARGET="$target" \
  bash "$script_root/scripts/install.sh" >/dev/null 2>&1; then
  echo "final replacement failure unexpectedly succeeded" >&2
  exit 1
fi
[[ -f "$temp_root/move-failed-once" ]]
if ! grep -Fq 'transaction old cli' "$transaction_dir/skilltape" || \
   ! grep -Fq 'transaction old api' "$transaction_dir/skilltape-console-api" || \
   ! grep -Fq 'transaction old UI' "$transaction_root/console/index.html" || \
   ! grep -Fq 'transaction old asset' "$transaction_root/console/assets/old.js"; then
  echo "final replacement failure did not restore the complete previous installation" >&2
  exit 1
fi

grep -Fq 'preserved receipt' "$transaction_root/data/receipt.json"
python3 - "$transaction_dir" <<'PY'
from pathlib import Path
import stat
import sys
for name in ("skilltape", "skilltape-console-api"):
    assert stat.S_IMODE((Path(sys.argv[1]) / name).stat().st_mode) == 0o700
PY
[[ ! -e "$transaction_root/.skilltape-install.lock" ]]

fixture_failure_install() {
  local fixture_root="$1" persistent="$2"
  PATH="$temp_root/mv-wrapper:$PATH" CURL_CA_BUNDLE="$certificate" \
    SKILLTAPE_FIXTURE_FAIL_DEST="$fixture_root/console" \
    SKILLTAPE_FIXTURE_ONCE="$fixture_root/failure-marker" SKILLTAPE_FIXTURE_ALWAYS="$persistent" \
    SKILLTAPE_RELEASE_BASE_URL="https://127.0.0.1:$port/releases/download" \
    SKILLTAPE_VERSION="$version" SKILLTAPE_INSTALL_DIR="$fixture_root/bin" SKILLTAPE_TARGET="$target" \
    bash "$script_root/scripts/install.sh" >"$fixture_root/result.log" 2>&1
}

fresh_root="$temp_root/fresh-failure"
mkdir -p "$fresh_root/bin"
if fixture_failure_install "$fresh_root" 0; then
  echo "fresh final replacement failure unexpectedly succeeded" >&2
  exit 1
fi
[[ -f "$fresh_root/failure-marker" ]]
[[ ! -e "$fresh_root/bin/skilltape" && ! -e "$fresh_root/bin/skilltape-console-api" && ! -e "$fresh_root/console" ]]
[[ ! -e "$fresh_root/.skilltape-install.lock" ]]

retained_root="$temp_root/retained-recovery"
mkdir -p "$retained_root/bin" "$retained_root/console/assets"
printf 'retained old cli\n' > "$retained_root/bin/skilltape"
printf 'retained old api\n' > "$retained_root/bin/skilltape-console-api"
printf 'retained old UI\n' > "$retained_root/console/index.html"
if fixture_failure_install "$retained_root" 1; then
  echo "blocked rollback unexpectedly succeeded" >&2
  exit 1
fi
grep -Fq 'retained old cli' "$retained_root/bin/skilltape"
grep -Fq 'retained old api' "$retained_root/bin/skilltape-console-api"
grep -Fq 'manual recovery' "$retained_root/result.log"
[[ -d "$retained_root/.skilltape-install.lock" ]]
backup_index="$(find "$retained_root" -path '*/previous-console/index.html' -print -quit)"
[[ -n "$backup_index" ]]
grep -Fq 'retained old UI' "$backup_index"

locked_root="$temp_root/locked-install"
mkdir -p "$locked_root/bin" "$locked_root/.skilltape-install.lock"
printf 'other owner\n' > "$locked_root/.skilltape-install.lock/owner"
if fixture_failure_install "$locked_root" 0; then
  echo "existing installation lock unexpectedly ignored" >&2
  exit 1
fi
grep -Fq 'other owner' "$locked_root/.skilltape-install.lock/owner"
[[ ! -e "$locked_root/bin/skilltape" ]]

# A committed installation remains successful if only lock cleanup is blocked.
cleanup_root="$temp_root/cleanup-warning"
mkdir -p "$cleanup_root/bin" "$temp_root/rmdir-wrapper"
cat > "$temp_root/rmdir-wrapper/rmdir" <<'SH'
#!/usr/bin/env bash
printf 'unexpected owner\n' > "$1/owner"
exec /bin/rmdir "$@"
SH
chmod 0755 "$temp_root/rmdir-wrapper/rmdir"
PATH="$temp_root/rmdir-wrapper:$PATH" CURL_CA_BUNDLE="$certificate" \
  SKILLTAPE_RELEASE_BASE_URL="https://127.0.0.1:$port/releases/download" \
  bash "$script_root/scripts/install.sh" "$version" "$cleanup_root/bin" "$target" >"$cleanup_root/result.log" 2>&1
grep -Fq 'new cli' "$cleanup_root/bin/skilltape"
grep -Fq 'new api' "$cleanup_root/bin/skilltape-console-api"
grep -Fq '<main>Console</main>' "$cleanup_root/console/index.html"
grep -Fq 'installation completed' "$cleanup_root/result.log"
grep -Fq 'unexpected owner' "$cleanup_root/.skilltape-install.lock/owner"

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
