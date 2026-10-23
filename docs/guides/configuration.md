# Optional configuration and local Console

Your first Skill needs no configuration. SkillTape does not automatically load
a `.env` file: set these values in the shell that runs the installer or Console.

## Capture and Tape resource limits

v0.2.3 limits Capture to 10,000 combined filesystem and
terminal events. Filesystem metadata is charged by occupied event slots and
owned string capacities, up to 16 MiB; vector capacity and allocator overhead
are additional and bounded by the event ceiling. The serialized terminal event
stream has a separate 16 MiB read budget. This is not a process RSS limit.

Exceeding a filesystem budget cancels the captured process and returns an error.
The final event merge moves its inputs instead of cloning them. It completes in
a sibling staging directory and uses the existing no-replace publisher, so
finalization errors preserve an existing destination and do not publish an
unfinished Tape. Split larger commands into separate captures.

Console event pages retain at most 100 events. Each request still validates the
complete event stream, with 16 MiB and 100,000-event ceilings and a 64 KiB Tape
manifest ceiling. Oversized or invalid data returns an error rather than a
partial successful page. Reads enforce the byte limit if the source grows;
these checks do not freeze files against another process changing them.

These limits are included in the v0.2.3 packages.

## Arguments and cancellation

`--command` names one executable. Pass each argument with `--arg`; spaces and
shell metacharacters are preserved literally. Use `--arg=--flag` for an argument
that starts with a hyphen. Arguments require an explicit `--command`:

```bash
skilltape capture greeting --command /bin/echo --arg "hello world" --yes
```

On Unix, Ctrl+C during Replay/Verify cancels the runner, waits for process-group
cleanup, and returns exit 5 with a `cancelled` JSON result. Verify saves the same
cancelled Receipt when `--receipt` is selected. It does not evaluate output
assertions or publish an incomplete `--delivery-dir`. Existing destinations are
preserved. Capture keeps its existing cancelled exit 130 and retained Tape.

Cancellation applies to execution; bounded synchronous input preparation must
finish before the signal is handled. Windows Replay/Verify remains unsupported.
Library callers can use `verify_run_with_cancel(request, token)`; `verify_run`
retains its existing signature. A cancelled Receipt has no evaluated assertions.
These changes are included in the published v0.2.3 assets.

## Installer settings

The installer reads `SKILLTAPE_VERSION`, `SKILLTAPE_RELEASE_BASE_URL`,
`SKILLTAPE_INSTALL_DIR`, and `SKILLTAPE_TARGET`. `SKILLTAPE_RELEASE_BASE_URL`
must be an HTTPS release download root. `SKILLTAPE_INSTALL_DIR` and
`SKILLTAPE_TARGET` are optional overrides; omit either to use the installer's
platform default.

Unix shells:

```bash
export SKILLTAPE_VERSION="0.4.1"
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
export SKILLTAPE_INSTALL_DIR="$HOME/.local/bin"
export SKILLTAPE_TARGET="x86_64-unknown-linux-gnu"
export SKILLTAPE_CONSOLE_API_BIN="/absolute/path/to/skilltape-console-api"
export SKILLTAPE_CONSOLE_UI_DIST="/absolute/path/to/console"
```

PowerShell:

```powershell
$env:SKILLTAPE_VERSION = "0.4.1"
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

The packaged Console timeline shows up to 50 Tapes and 100 events per page.
Use Previous/Next tapes to choose another group of captures and Previous/Next
events to inspect the rest of a selected Tape. Changing the Tape resets the
event page. The URL preserves both page positions for reload and browser
back/forward navigation; pages replace earlier data rather than accumulating it.
If a page request fails, Retry request reloads that same page. A finished
capture is shown as Finished even when its completion timestamp is zero.
These controls are included in the v0.2.3 packages.

Return to the [installation guide](installation.md) for supported targets and
updates, or the [Quickstart](quickstart.md) to create your first Skill.

### Console read errors and recovery

The Console checks the fields it renders from saved timelines and compiled packages. Out-of-range timestamps, malformed workflow steps, permission shapes and file metadata show an error with a retry action. Check the saved Tape or package with the current CLI before retrying. If the connection fails, start the local Console again.

An empty policy plan is labelled as having no recorded checks; a filter with no denials remains a separate state. The Export guide lists CLI commands, and the Console remains read-only. The 2026-10-23 frontend check covered 34 Console cases plus a production build. Existing saved Tapes, packages, receipts, two delivery bundles and their run-condition records were reread without replay. Twenty navigation changes retained one document, with heap samples of 8,212,728 / 7,535,948 / 7,935,608 bytes; this is a bounded local observation, not a long-duration load result.
