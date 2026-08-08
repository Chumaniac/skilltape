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

The public `v0.1.0` release supports these targets:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

The fixed installers fetch the selected archive and `checksums.txt`, verify the
archive SHA-256, validate the CLI, API companion, and Console UI, and stage all
assets before replacement. A download, checksum, extraction, or staging failure
leaves an existing installation untouched.

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
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
SKILLTAPE_VERSION="0.1.0" ./scripts/install.sh
```

Pass a version, installation directory, and target explicitly to install in a
custom Unix location:

```bash
SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download" \
  ./scripts/install.sh 0.1.0 "$HOME/.local/bin" "aarch64-apple-darwin"
```

On Windows, set the same values in PowerShell before running `install.ps1`:

```powershell
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/Chumaniac/skilltape/releases/download"
$env:SKILLTAPE_VERSION = "0.1.0"
$env:SKILLTAPE_INSTALL_DIR = "$env:LOCALAPPDATA\SkillTape\bin"
$env:SKILLTAPE_TARGET = "x86_64-pc-windows-msvc"
.\scripts\install.ps1
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
