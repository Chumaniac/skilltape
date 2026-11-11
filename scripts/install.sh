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
stage_dir=""
backup_dir=""
lock_dir=""
own_lock=0
transaction_active=0
moving_previous=0
saved_cli=0
saved_api=0
saved_console=0
attempted_cli=0
attempted_api=0
attempted_console=0

exists() { [[ -e "$1" || -L "$1" ]]; }

restore_component() {
  local target_path="$1" name="$2" attempted="$3" saved="$4"
  if [[ "$attempted" == 1 ]] && exists "$target_path"; then
    # Preserve even partially moved new assets if restoring the old copy fails.
    mv "$target_path" "$backup_dir/failed-$name" || return 1
  fi
  if [[ "$saved" == 1 ]]; then
    if exists "$target_path"; then return 1; fi
    mv "$backup_dir/previous-$name" "$target_path" || return 1
  fi
}

cleanup() {
  local status=$? recovery_needed=0
  trap - EXIT INT TERM
  set +e
  if [[ "$transaction_active" == 1 ]]; then
    restore_component "$install_parent/console" console "$attempted_console" "$saved_console" || recovery_needed=1
    restore_component "$install_dir/skilltape-console-api" api "$attempted_api" "$saved_api" || recovery_needed=1
    restore_component "$install_dir/skilltape" cli "$attempted_cli" "$saved_cli" || recovery_needed=1
    [[ "$moving_previous" == 0 ]] || recovery_needed=1
    [[ "$status" != 0 ]] || status=1
  fi
  [[ -z "$stage_dir" ]] || rm -rf "$stage_dir"
  rm -rf "$tmp_dir"
  if [[ "$recovery_needed" == 1 ]]; then
    echo "installation rollback needs manual recovery; preserved assets: $backup_dir; lock: $lock_dir" >&2
    status=1
  else
    [[ -z "$backup_dir" ]] || rm -rf "$backup_dir"
    if [[ "$own_lock" == 1 ]]; then
      # Do not remove unexpected files added to the lock by another process.
      rm -f "$lock_dir/pid"
      rmdir "$lock_dir" || echo "installation completed; inspect the retained lock before retrying: $lock_dir" >&2
    fi
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

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

expected="$(awk -v asset="$asset" '
  {
    name = $2
    sub(/^\*/, "", name)
    sub(/^\.\//, "", name)
    if (name == asset) { print $1; exit }
  }
' "$checksums")"
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
lock_dir="$install_parent/.skilltape-install.lock"
if ! mkdir "$lock_dir"; then
  echo "installation lock already exists; inspect it before retrying: $lock_dir" >&2
  exit 1
fi
own_lock=1
chmod 0700 "$lock_dir"
printf '%s\n' "$$" > "$lock_dir/pid"
for executable in "$install_dir/skilltape" "$install_dir/skilltape-console-api"; do
  if exists "$executable" && [[ ! -f "$executable" && ! -L "$executable" ]]; then
    echo "existing executable must be a regular file or symlink: $executable" >&2
    exit 1
  fi
done
if exists "$install_parent/console" && [[ ! -d "$install_parent/console" && ! -L "$install_parent/console" ]]; then
  echo "existing Console must be a directory or symlink" >&2
  exit 1
fi
stage_dir="$(mktemp -d "$install_parent/.skilltape-stage.XXXXXX")"
backup_dir="$(mktemp -d "$install_parent/.skilltape-rollback.XXXXXX")"
chmod 0700 "$stage_dir" "$backup_dir"
staged_cli="$stage_dir/skilltape"
staged_api="$stage_dir/skilltape-console-api"
staged_console="$stage_dir/console"
cp "$candidate" "$staged_cli"
cp "$candidate_api" "$staged_api"
mkdir -p "$staged_console"
cp -R "$console_source/." "$staged_console/"
chmod 0755 "$staged_cli" "$staged_api"
[[ -f "$staged_cli" && -f "$staged_api" && -f "$staged_console/index.html" ]] || {
  echo "staged release assets are incomplete" >&2
  exit 1
}

transaction_active=1
if exists "$install_dir/skilltape"; then
  moving_previous=1
  mv "$install_dir/skilltape" "$backup_dir/previous-cli"
  saved_cli=1
  moving_previous=0
fi
if exists "$install_dir/skilltape-console-api"; then
  moving_previous=1
  mv "$install_dir/skilltape-console-api" "$backup_dir/previous-api"
  saved_api=1
  moving_previous=0
fi
if exists "$install_parent/console"; then
  moving_previous=1
  mv "$install_parent/console" "$backup_dir/previous-console"
  saved_console=1
  moving_previous=0
fi
attempted_cli=1
mv "$staged_cli" "$install_dir/skilltape"
attempted_api=1
mv "$staged_api" "$install_dir/skilltape-console-api"
attempted_console=1
mv "$staged_console" "$install_parent/console"
transaction_active=0

echo "Installed skilltape $version for $target at $install_dir/skilltape"
