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
staged=""
cleanup() {
  if [[ -n "$staged" && -e "$staged" ]]; then
    rm -f -- "$staged"
  fi
  rm -rf -- "$tmp_dir"
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
tar --extract --gzip --file "$archive" --directory "$extract_dir" --no-same-owner --no-overwrite-dir
candidate="$(find "$extract_dir" -type f -name skilltape -print -quit)"
if [[ -z "$candidate" ]]; then
  echo "release archive does not contain a skilltape binary" >&2
  exit 1
fi

mkdir -p "$install_dir"
staged="$install_dir/.skilltape.tmp.$$"
cp -- "$candidate" "$staged"
chmod 0755 "$staged"
mv -f -- "$staged" "$install_dir/skilltape"
staged=""

echo "Installed skilltape $version for $target at $install_dir/skilltape"
