#!/usr/bin/env bash
set -Eeuo pipefail

if [[ $# -ne 1 || ! -x "$1" ]]; then
  echo "usage: $0 <executable-skilltape-path>" >&2
  exit 2
fi

skilltape_bin="$1"
demo_root="$(mktemp -d "${TMPDIR:-/tmp}/skilltape-quickstart.XXXXXX")"
workspace="$demo_root/workspace"
tape="$demo_root/tape"
skill="$demo_root/skill"
receipt="$demo_root/receipt.json"
cleanup() {
  rm -rf "$demo_root"
}
trap cleanup EXIT

mkdir -p "$workspace"

"$skilltape_bin" capture demo --workspace "$workspace" --command /bin/echo \
  --output "$tape" --yes --json > "$demo_root/capture.json"
"$skilltape_bin" compile "$tape" --output "$skill" > "$demo_root/compile.txt"
"$skilltape_bin" lint "$skill" --strict --json > "$demo_root/lint.json"
"$skilltape_bin" verify "$skill" --receipt "$receipt" --json > "$demo_root/receipt-output.json"

python3 - "$demo_root" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
capture = json.loads((root / "capture.json").read_text())
lint = json.loads((root / "lint.json").read_text())
receipt_output = json.loads((root / "receipt-output.json").read_text())

assert capture["ok"] is True
assert capture["name"] == "demo"
assert capture["event_count"] >= 4
assert lint["errors"] == []
assert receipt_output["schema"] == "skilltape.dev/receipt/v1"
assert receipt_output["status"] == "succeeded"
assert receipt_output == json.loads((root / "receipt.json").read_text())
assert (root / "skill" / "SKILL.md").is_file()
assert (root / "skill" / "workflow.yaml").is_file()
PY

echo "quickstart journey passed"
