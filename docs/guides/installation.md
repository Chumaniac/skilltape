# Install and update SkillTape

For your first Skill, start with the [Quickstart](quickstart.md). The native
release is the default path: it installs a checksum-verified CLI, its Console
API companion, and the packaged Console UI without requiring a source build.

## Install a native release

Release assets use these names:

```text
skilltape-v<version>-<target>.tar.gz   # macOS/Linux
skilltape-v<version>-<target>.zip      # Windows
checksums.txt
```

The `v0.5.2` release supports these targets:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

The fixed installers fetch the selected archive and `checksums.txt`, verify the
archive SHA-256, validate the CLI, API companion, and Console UI, and stage all
assets before replacement. A download, checksum, extraction, or staging failure
leaves an existing installation untouched.

The pinned installers additionally back up the CLI, API and UI as one
replacement transaction. A final-move failure restores the complete previous set;
a failed fresh install removes its partial set. If restoration is blocked, the
installer preserves its backup and installation lock and reports the recovery
location. Inspect those files before removing the lock or retrying. Process
termination without cleanup (for example, SIGKILL or power loss) still requires
manual recovery. Existing historical installers and release archives remain available without replacement.

Linux needs `bwrap`/Bubblewrap and an available user namespace for Replay and
Verify; Debian and Ubuntu users can install it with `sudo apt-get install
bubblewrap`. macOS uses `/usr/bin/sandbox-exec`. Without the corresponding
sandbox, Capture, Compile, Lint, and Export remain usable while Replay and
Verify fail closed. Windows supports Capture, Compile, Lint, and Export;
Replay and Verify fail closed until an equivalent restricted executor exists.

The [Quickstart](quickstart.md) downloads a fixed installer source and runs the
public release flow. To choose a trusted mirror, a different install directory,
or a supported target, see [optional configuration](configuration.md).

## Update or choose a custom directory

Run the installer again with the version you want. It safely replaces the
installed files only after the new archive has passed all checks:

```bash
installer_path="$(mktemp)"
curl --fail --location --silent --show-error --output "$installer_path" \
  "https://raw.githubusercontent.com/Chumaniac/skilltape/aa0380d0ece4c385eb3e7e369009a5a73b48d0c7/scripts/install.sh"
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
export SKILLTAPE_VERSION="0.5.2"
bash "$installer_path"
```

Pass a version, installation directory, and target explicitly to install in a
custom Unix location:

```bash
installer_path="$(mktemp)"
curl --fail --location --silent --show-error --output "$installer_path" \
  "https://raw.githubusercontent.com/Chumaniac/skilltape/aa0380d0ece4c385eb3e7e369009a5a73b48d0c7/scripts/install.sh"
SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download" \
  bash "$installer_path" 0.5.2 "$HOME/.local/bin" "aarch64-apple-darwin"
```

On Windows, download the fixed installer source before setting the same values
in PowerShell:

```powershell
$installerPath = Join-Path $env:TEMP "skilltape-install.ps1"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/Chumaniac/skilltape/aa0380d0ece4c385eb3e7e369009a5a73b48d0c7/scripts/install.ps1" -OutFile $installerPath
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/Chumaniac/skilltape/releases/download"
$env:SKILLTAPE_VERSION = "0.5.2"
$env:SKILLTAPE_INSTALL_DIR = "$env:LOCALAPPDATA\SkillTape\bin"
$env:SKILLTAPE_TARGET = "x86_64-pc-windows-msvc"
& $installerPath
```

## Use the packaged Console

The native package includes `skilltape-console-api` and `console/index.html`.
`skilltape console --workspace .` discovers those packaged assets by default.
`SKILLTAPE_CONSOLE_API_BIN` and `SKILLTAPE_CONSOLE_UI_DIST` are optional local
overrides for intentionally using different packaged assets; they are not
required setup. See [configuration](configuration.md) for the shell templates.

## Build from source (contributors)

If you are contributing or need an unreleased build, install Rust 1.97.1 and
build from the repository root:

```bash
cargo build --locked --release -p skilltape-cli -p skilltape-console-api
cargo install --locked --path crates/skilltape-cli
```

To build the Console UI from source before running `skilltape console`, use:

```bash
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
```


## Current-source candidates

Use the [candidate identity guide](source-candidates.md) to build and inspect one clean source revision. `--require-identity` refuses mixed CLI/API/UI artifacts before packaging. This does not publish or update the fixed public release; retain old installations and successful data.

The Apple Silicon application is in the same [v0.5.2 Release](https://github.com/Chumaniac/skilltape/releases/tag/v0.5.2). Verify its ZIP against `checksums.txt`; its source manifest records the actual minimum macOS version. It is ad-hoc signed and is not Apple notarized. [Stopped-workspace recovery](workspace-recovery.md) uses a new directory and never overwrites the original.
