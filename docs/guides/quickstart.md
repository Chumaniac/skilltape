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

On Linux, Verify requires `bwrap`/Bubblewrap and an available user namespace.
Debian and Ubuntu users can install it with `sudo apt-get install bubblewrap`.
macOS uses `/usr/bin/sandbox-exec`.

## If Verify cannot start

Install or enable the required local sandbox before trying Verify again. If it
is unavailable, Capture, Compile, Lint, and Export remain usable; Verify fails
closed rather than pretending to isolate the replay. Windows currently supports
those non-execution commands, while Replay and Verify fail closed until an
equivalent restricted executor is integrated.

## What the result means

`"status":"succeeded"` means the declared Skill replay completed in the
available restricted executor and the Receipt recorded that result. It does not
make future edits or undeclared permissions safe: review the Skill and its
permissions before sharing it.

## Next steps

Read the [installation guide](installation.md) for supported platforms, update
details, source builds, and release archives. Inspect the
[minimal Skill example](../../examples/minimal-skill/README.md) to see the
package you can review and adapt.
