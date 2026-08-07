#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  cat >&2 <<'USAGE'
Usage: install.sh <version> [install-dir] [target]

Required:
  SKILLTAPE_RELEASE_BASE_URL  Release URL ending in /releases/download

Version, install directory, and target can also be supplied through
SKILLTAPE_VERSION, SKILLTAPE_INSTALL_DIR, and SKILLTAPE_TARGET.
USAGE
}

version="${1:-${SKILLTAPE_VERSION:-}}"
install_dir="${2:-${SKILLTAPE_INSTALL_DIR:-${HOME}/.local/bin}}"
target="${3:-${SKILLTAPE_TARGET:-}}"
release_base="${SKILLTAPE_RELEASE_BASE_URL:-}"

if [[ -z "$version" || -z "$release_base" ]]; then
  usage
  exit 2
fi

version="${version#v}"

if [[ -z "$target" ]]; then
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) target="aarch64-apple-darwin" ;;
    Darwin:x86_64) target="x86_64-apple-darwin" ;;
    Linux:x86_64|Linux:amd64) target="x86_64-unknown-linux-gnu" ;;
    Linux:aarch64|Linux:arm64) target="aarch64-unknown-linux-gnu" ;;
    *)
      echo "cannot infer a Rust release target; set SKILLTAPE_TARGET" >&2
      exit 2
      ;;
  esac
fi

case "$target" in
  *windows*)
    echo "install.sh supports Unix archives only; use scripts/install.ps1 on Windows" >&2
    exit 2
    ;;
esac

asset="skilltape-v${version}-${target}.tar.gz"
release_root="${release_base%/}/v${version}"
archive_url="${release_root}/${asset}"
checksums_url="${release_root}/checksums.txt"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/skilltape-install.XXXXXX")"
staged_cli=""
staged_api=""
staged_console=""
cleanup() {
  [[ -z "$staged_cli" || ! -e "$staged_cli" ]] || rm -f "$staged_cli"
  [[ -z "$staged_api" || ! -e "$staged_api" ]] || rm -f "$staged_api"
  [[ -z "$staged_console" || ! -e "$staged_console" ]] || rm -rf "$staged_console"
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

download() {
  local url="$1"
  local destination="$2"
  if command -v curl >/dev/null 2>&1; then
    curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error \
      --output "$destination" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget --https-only --quiet --output-document "$destination" "$url"
  else
    echo "curl or wget is required" >&2
    return 1
  fi
}

archive="$tmp_dir/$asset"
checksums="$tmp_dir/checksums.txt"
download "$archive_url" "$archive"
download "$checksums_url" "$checksums"

expected="$(awk -v asset="$asset" '$2 == asset || $2 == ("*" asset) { print $1; exit }' "$checksums")"
if [[ ! "$expected" =~ ^[[:xdigit:]]{64}$ ]]; then
  echo "checksums.txt has no valid SHA-256 entry for $asset" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$archive" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
else
  echo "sha256sum or shasum is required" >&2
  exit 1
fi

actual_lower="$(printf '%s' "$actual" | tr '[:upper:]' '[:lower:]')"
expected_lower="$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')"
if [[ "$actual_lower" != "$expected_lower" ]]; then
  echo "checksum mismatch for $asset" >&2
  exit 1
fi

extract_dir="$tmp_dir/extracted"
mkdir -p "$extract_dir"
tar -xzf "$archive" -C "$extract_dir"
candidate="$(find "$extract_dir" -type f -name skilltape -print -quit)"
if [[ -z "$candidate" ]]; then
  echo "release archive does not contain a skilltape binary" >&2
  exit 1
fi
candidate_api="$(find "$extract_dir" -type f -name skilltape-console-api -print -quit)"
if [[ -z "$candidate_api" ]]; then
  echo "release archive does not contain a skilltape-console-api binary" >&2
  exit 1
fi
console_index="$(find "$extract_dir" -type f -path '*/console/index.html' -print -quit)"
if [[ -z "$console_index" ]]; then
  echo "release archive does not contain console/index.html" >&2
  exit 1
fi
console_source="${console_index%/index.html}"
if [[ -L "$console_source" || ! -d "$console_source" ]]; then
  echo "release archive contains an unsafe Console UI directory" >&2
  exit 1
fi
if [[ -n "$(find "$console_source" -type l -print -quit)" ]]; then
  echo "release archive contains a symlink in the Console UI" >&2
  exit 1
fi
console_assets="$console_source/assets"
if [[ -L "$console_assets" || ! -d "$console_assets" || -z "$(find "$console_assets" -type f -print -quit)" ]]; then
  echo "release archive does not contain regular Console UI assets" >&2
  exit 1
fi

mkdir -p "$install_dir"
if [[ -L "$install_dir" ]]; then
  echo "install directory must not be a symlink: $install_dir" >&2
  exit 1
fi
install_parent="$(dirname "$install_dir")"
mkdir -p "$install_parent"
staged_cli="$install_dir/.skilltape.tmp.$$"
staged_api="$install_dir/.skilltape-console-api.tmp.$$"
staged_console="$install_parent/.skilltape-console.tmp.$$"
cp "$candidate" "$staged_cli"
cp "$candidate_api" "$staged_api"
mkdir -p "$staged_console"
cp -R "$console_source/." "$staged_console/"
chmod 0755 "$staged_cli" "$staged_api"
[[ -f "$staged_cli" && -f "$staged_api" && -f "$staged_console/index.html" ]] || {
  echo "staged release assets are incomplete" >&2
  exit 1
}

mv -f "$staged_cli" "$install_dir/skilltape"
staged_cli=""
mv -f "$staged_api" "$install_dir/skilltape-console-api"
staged_api=""
previous_console="$tmp_dir/previous-console"
if [[ -e "$install_parent/console" || -L "$install_parent/console" ]]; then
  mv "$install_parent/console" "$previous_console"
fi
if ! mv "$staged_console" "$install_parent/console"; then
  [[ -e "$previous_console" ]] && mv "$previous_console" "$install_parent/console"
  echo "failed to install Console UI" >&2
  exit 1
fi
staged_console=""

echo "Installed skilltape $version for $target at $install_dir/skilltape"
