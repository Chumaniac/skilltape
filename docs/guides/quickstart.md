# Get a verified Skill in five minutes

SkillTape turns a local command into a reviewable Skill, then produces a
Receipt from an isolated replay. The first result is local: it needs no account,
API key, model provider, or configuration file.

## What you will get

You will Capture `/bin/echo`, Compile it into a Skill, run strict Lint, and
Verify the Skill. A successful Receipt ends with `"status":"succeeded"`.

## macOS and Linux

Install the fixed public v0.1.0 release without piping a download into a shell.
The installer source is pinned to commit
`beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6`, and the public release base URL is
explicit.

```bash
SKILLTAPE_VERSION="0.1.0"
SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
installer_path="$(mktemp)"
curl --fail --location --silent --show-error --output "$installer_path" \
  "https://raw.githubusercontent.com/Chumaniac/skilltape/beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6/scripts/install.sh"
chmod +x "$installer_path"
export SKILLTAPE_VERSION SKILLTAPE_RELEASE_BASE_URL
bash "$installer_path"
export PATH="$HOME/.local/bin:$PATH"
skilltape --help
```

Capture and verify a harmless local command:

```bash
workspace="$(mktemp -d)"

skilltape capture demo --workspace "$workspace" --command /bin/echo --output "$workspace/tape" --yes --json
skilltape compile "$workspace/tape" --output "$workspace/skill"
skilltape lint "$workspace/skill" --strict --json
skilltape verify "$workspace/skill" --receipt "$workspace/receipt.json" --json
```

On Linux, Replay and Verify require `bwrap`/Bubblewrap and an available user
namespace. Debian and Ubuntu users can install it with
`sudo apt-get install bubblewrap`. macOS uses `/usr/bin/sandbox-exec` for
Replay and Verify.

## If Replay or Verify cannot start

Install or enable the required local sandbox before trying Replay or Verify
again. If it is unavailable, Capture, Compile, Lint, and Export remain usable;
Replay and Verify fail closed rather than pretending to isolate the replay.
Windows currently supports those non-execution commands, while Replay and Verify
fail closed until an equivalent restricted executor is integrated.

## Windows: create and export a Skill

Use PowerShell to download the fixed installer source and install the public
release. This path does not require a compiler or a checkout of this repository.

```powershell
$installerPath = Join-Path $env:TEMP "skilltape-install.ps1"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/Chumaniac/skilltape/beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6/scripts/install.ps1" -OutFile $installerPath
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/Chumaniac/skilltape/releases/download"
$env:SKILLTAPE_VERSION = "0.1.0"
$env:SKILLTAPE_TARGET = "x86_64-pc-windows-msvc"
& $installerPath
$env:PATH = "$env:LOCALAPPDATA\SkillTape\bin;$env:PATH"
skilltape --help
```

Capture a harmless Windows command, then Compile, Lint, and Export the Skill:

```powershell
$workspace = Join-Path $env:TEMP "skilltape-quickstart-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $workspace | Out-Null

skilltape capture demo --workspace $workspace --command whoami.exe --output "$workspace\tape" --yes --json
skilltape compile "$workspace\tape" --output "$workspace\skill"
skilltape lint "$workspace\skill" --strict --json
skilltape export "$workspace\skill" --target generic --output "$workspace\exported-skill" --json
```

Replay/Verify fail closed on Windows, so this path deliberately stops after Export.

## What the result means

`"status":"succeeded"` means the declared Skill replay completed in the
available restricted executor and the Receipt recorded that result. It does not
make future edits or undeclared permissions safe: review the Skill and its
permissions before sharing it.

## Next steps

Read the [installation guide](installation.md) for supported platforms, updates,
source builds, and release archives. See [optional configuration](configuration.md)
for installer paths and local Console overrides. Inspect the [minimal Skill
example](../../examples/minimal-skill/README.md) to see the package you can
review and adapt.
