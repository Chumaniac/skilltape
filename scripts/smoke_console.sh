#!/usr/bin/env bash
set -Eeuo pipefail

workspace="${1:-}"
api_binary="${SKILLTAPE_CONSOLE_API_BIN:-}"
ui_dist="${SKILLTAPE_CONSOLE_UI_DIST:-}"
if [[ -z "$workspace" || -z "$api_binary" || -z "$ui_dist" ]]; then
  echo "usage: SKILLTAPE_CONSOLE_API_BIN=... SKILLTAPE_CONSOLE_UI_DIST=... $0 WORKSPACE" >&2
  exit 2
fi
[[ -d "$workspace" ]] || { echo "workspace is not a directory: $workspace" >&2; exit 2; }
[[ -f "$api_binary" ]] || { echo "API binary is missing: $api_binary" >&2; exit 2; }
[[ -f "$ui_dist/index.html" ]] || { echo "UI index is missing: $ui_dist/index.html" >&2; exit 2; }

log_file="$(mktemp "${TMPDIR:-/tmp}/skilltape-console-smoke.XXXXXX")"
child_pid=""
cleanup() {
  if [[ -n "$child_pid" ]]; then
    kill "$child_pid" 2>/dev/null || true
    wait "$child_pid" 2>/dev/null || true
  fi
  rm -f "$log_file"
}
trap cleanup EXIT

"$api_binary" \
  --workspace "$workspace" \
  --bind 127.0.0.1 \
  --port 0 \
  --static-root "$ui_dist" \
  >"$log_file" 2>&1 &
child_pid=$!

ready_url=""
for _ in {1..100}; do
  if grep -Fq 'SkillTape Console API listening at ' "$log_file"; then
    ready_url="$(sed -n 's/^SkillTape Console API listening at //p' "$log_file" | head -n 1)"
    break
  fi
  if ! kill -0 "$child_pid" 2>/dev/null; then
    cat "$log_file" >&2
    echo "Console API exited before readiness" >&2
    exit 1
  fi
  sleep 0.05
done
if [[ -z "$ready_url" ]]; then
  cat "$log_file" >&2
  echo "Console API did not become ready" >&2
  exit 1
fi
case "$ready_url" in
  http://127.0.0.1:*) ;;
  *) echo "Console API did not bind to loopback: $ready_url" >&2; exit 1 ;;
esac

python3 - "$ready_url" <<'PY'
import http.client
import json
import sys
from urllib.parse import urlsplit

parts = urlsplit(sys.argv[1])


def get(path: str) -> str:
    connection = http.client.HTTPConnection(parts.hostname, parts.port, timeout=2)
    connection.request("GET", path, headers={"Connection": "close"})
    response = connection.getresponse()
    body = response.read().decode("utf-8")
    connection.close()
    if response.status != 200:
        raise RuntimeError(f"GET {path} returned HTTP {response.status}: {body}")
    return body


document = json.loads(get("/api/v1/workspaces"))
assert document["schema"] == "skilltape.dev/console/v1"
assert document["items"]

html = get("/")
assert "SkillTape Console" in html
PY

echo "Console API/UI smoke passed at $ready_url"
