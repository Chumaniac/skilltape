# Optional configuration and local Console

Your first Skill needs no configuration. SkillTape does not automatically load
a `.env` file: set these values in the shell that runs the installer or Console.

## Installer settings

The installer reads `SKILLTAPE_VERSION`, `SKILLTAPE_RELEASE_BASE_URL`,
`SKILLTAPE_INSTALL_DIR`, and `SKILLTAPE_TARGET`. `SKILLTAPE_RELEASE_BASE_URL`
must be an HTTPS release download root. `SKILLTAPE_INSTALL_DIR` and
`SKILLTAPE_TARGET` are optional overrides; omit either to use the installer's
platform default.

Unix shells:

```bash
export SKILLTAPE_VERSION="0.1.0"
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
export SKILLTAPE_INSTALL_DIR="$HOME/.local/bin"
export SKILLTAPE_TARGET="x86_64-unknown-linux-gnu"
export SKILLTAPE_CONSOLE_API_BIN="/absolute/path/to/skilltape-console-api"
export SKILLTAPE_CONSOLE_UI_DIST="/absolute/path/to/console"
```

PowerShell:

```powershell
$env:SKILLTAPE_VERSION = "0.1.0"
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/Chumaniac/skilltape/releases/download"
$env:SKILLTAPE_INSTALL_DIR = "$env:LOCALAPPDATA\SkillTape\bin"
$env:SKILLTAPE_TARGET = "x86_64-pc-windows-msvc"
$env:SKILLTAPE_CONSOLE_API_BIN = "C:\absolute\path\to\skilltape-console-api.exe"
$env:SKILLTAPE_CONSOLE_UI_DIST = "C:\absolute\path\to\console"
```

## Local Console overrides

`SKILLTAPE_CONSOLE_API_BIN` and `SKILLTAPE_CONSOLE_UI_DIST` are optional
packaged-asset overrides, not required setup. Set them only when you
intentionally want `skilltape console` to use a different Console API binary or
UI directory than the installed package discovers. The UI directory must contain
`index.html`; the API path must point to the executable for your platform.

After setting any intended overrides, start the read-only local viewer from a
workspace directory:

```bash
skilltape console --workspace .
```

Return to the [installation guide](installation.md) for supported targets and
updates, or the [Quickstart](quickstart.md) to create your first Skill.
