# SkillTape

> **Beta** — Turn a real local workflow into a reviewable Agent Skill you can replay and verify before you share it.

SkillTape captures a command you already run, turns it into a reviewable Skill,
and produces a Receipt that tells you whether the isolated replay succeeded.
It runs locally; the first result needs no account, API key, model provider, or
configuration file.

![Terminal output from SkillTape Capture, Compile, Lint, and Verify](docs/assets/quickstart-terminal.svg)

[Watch the 30-second terminal demo](docs/assets/quickstart-terminal.txt) · [Download v0.2.3](https://github.com/Chumaniac/skilltape/releases/tag/v0.2.3)

## Verified public release

[SkillTape v0.2.3](https://github.com/Chumaniac/skilltape/releases/tag/v0.2.3)
is publicly available as native CLI, Console API and Console UI packages.
[Release run 38862770537](https://github.com/Chumaniac/skilltape/actions/runs/38862770537)
passed Linux, Intel Mac, Apple Silicon Mac and Windows builds, required
provenance/SBOM/checksums and the published Windows installer smoke check.
All eight downloaded archive/SBOM checksums matched. An isolated native Mac
installation passed required-output success/failure and packaged Console checks.

Windows Replay/Verify still fail closed. The terminal demo records the historical
v0.1.0 journey; the commands below install v0.2.3. Earlier releases remain available.

## New in v0.2.3

- Require output files with repeatable `verify --expect-file PATH`; missing files
  fail verification and leave the delivery unpublished. See the example below.
- Inspect and compare real saved bundles in the Console's
  [Delivery workbench](docs/reference/saved-deliveries.md), including actual file
  hashes and changes. Native physical inspection currently requires POSIX.
- Pass literal Capture arguments with `--arg` and cancel Unix Replay/Verify with
  orderly cleanup and exit 5. See [arguments and cancellation](docs/guides/configuration.md#arguments-and-cancellation).
- Use bounded stable exports, optional `export --receipt` association and
  complete no-overwrite delivery publication. The
  [multi-organization example](examples/verified-tenant-delivery/README.md)
  pairs actual local output with independent composite-key checks.

## Get a verified Skill in five minutes

Install the fixed public v0.2.3 release without piping a download into a shell.
The installer below is fetched from commit
`9f6bec579258baa63234139af0fae650d9efdbd2`, and the release base URL stays
explicit.

```bash
SKILLTAPE_VERSION="0.2.3"
SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
installer_path="$(mktemp)"
curl --fail --location --silent --show-error --output "$installer_path" \
  "https://raw.githubusercontent.com/Chumaniac/skilltape/9f6bec579258baa63234139af0fae650d9efdbd2/scripts/install.sh"
chmod +x "$installer_path"
export SKILLTAPE_VERSION SKILLTAPE_RELEASE_BASE_URL
bash "$installer_path"
export PATH="$HOME/.local/bin:$PATH"
skilltape --help
```

Then capture a harmless local command and get a verified result:

```bash
workspace="$(mktemp -d)"

skilltape capture demo --workspace "$workspace" --command /bin/echo --output "$workspace/tape" --yes --json
skilltape compile "$workspace/tape" --output "$workspace/skill"
skilltape lint "$workspace/skill" --strict --json
skilltape verify "$workspace/skill" --receipt "$workspace/receipt.json" --json
```

The final Receipt reports `"status":"succeeded"` when the isolated replay
completed successfully.

## What works today

On Linux, Replay and Verify require `bwrap`/Bubblewrap and an available user
namespace. On macOS, Replay and Verify use the system
`/usr/bin/sandbox-exec`. If that restricted executor is unavailable, Capture,
Compile, Lint, and Export remain usable while Replay and Verify fail closed.
Windows supports Capture, Compile, Lint, and Export; Replay and Verify
intentionally fail closed until an equivalent restricted executor is integrated.

The packaged Console supports previous/next navigation through Tapes and events,
keeping only the current page in memory. See the [Console guide](docs/guides/configuration.md#local-console-overrides)
for startup details. This improvement is included in v0.2.3.

v0.2.3 also bounds server-side Tape event reads to
16 MiB and 100,000 events while validating the complete stream. Capture merges
at most 10,000 events, charges allocated filesystem metadata against a 16 MiB
budget, and rejects oversized terminal event streams before merging. These are
retained-data budgets, not a process RSS guarantee. Complete Tapes are published
without replacing an existing destination; budget or finalization failures do
not publish a partial Tape. See the [resource limits](docs/guides/configuration.md#capture-and-tape-resource-limits).

For retained deliveries, run `skilltape console --workspace /path/to/saved-parent`
and choose **Delivery workbench**. This interface discovers actual
bundles directly and provides bounded file checks and review actions. It never
executes a workflow, edits files or declares business requirements accepted.
These features are included in the public v0.2.3 packages.

The current source also provides a saved-Receipt picker and an assertion review
with **All checks** and **Needs attention** filters. It reads the actual v1
`kind` field, shows explicit pass/fail labels, and keeps the run outcome visible
while filtering. See [review saved verification results](docs/guides/receipt-review.md)
for an operational example. This UI update is not in the existing v0.2.3 archives.

v0.2.3 also bounds Replay/Verify inputs to 10,000 entries, depth 64,
16 MiB per file, and 64 MiB in total. Hashing and staging reject detected changes
instead of reading a growing file without a limit. See the
[input integrity boundaries](docs/guides/domain-workflows.md) for platform and
concurrent-change limits. These checks are included in v0.2.3.

## Use SkillTape when

- You have a local command or workflow worth making repeatable and reviewable.
- You want to inspect the generated Skill before someone else runs it.
- You need a bounded Receipt from an isolated replay before sharing the result.

## It is not yet for

- Replacing a general-purpose agent runtime or model provider.
- Claiming isolated replay on Windows before its restricted executor exists.
- Treating unreviewed Tapes, workflows, or permissions as safe to run.

## Learn more

- [Expansion and deep optimization roadmap](docs/deep-optimization-roadmap.md) — concrete priorities, domain workflows, limits and release gates.
- [Stable export and Receipt association](docs/guides/export-evidence.md) — consistent bytes and explicitly unauthenticated evidence.
- [Verified local delivery](docs/guides/verified-delivery.md) — persistent artifacts, Receipt/file hashes, and an independently checked order-summary example.
- [Project overview](https://chumanic.com/projects/skilltape/) — Chinese introduction and current platform boundaries.
- [Domain workflow examples](docs/guides/domain-workflows.md) — source examples for code-review evidence, knowledge references, data-export integrity, and incident-review materials; no external execution or platform connection.
- [Quickstart](docs/guides/quickstart.md) — the same first verified result with troubleshooting.
- [Installation](docs/guides/installation.md) — platform, update, source-build, and release details.
- [Configuration](docs/guides/configuration.md) — optional release, installation, and Console settings.
- [Minimal Skill example](examples/minimal-skill/README.md) — a small valid package to inspect.
- [Security](SECURITY.md) — sandbox boundaries, redaction, and responsible disclosure.
- [Contributing](CONTRIBUTING.md) — development and contribution guidance.

### Require output files during Verify

With v0.2.3, use `skilltape verify ./skill --expect-file result.csv --expect-file summary.txt --json`. Each path is relative to the output root. Verify records each requirement in the Receipt and fails with exit code 3 if a required regular file is missing. A failed Verify does not publish a delivery directory. Unsafe paths fail before Replay; at most 64 paths of 1024 bytes each are accepted. This checks file existence, not business correctness or authenticated provenance. These requirements are available in the v0.2.3 CLI.

### Require exact output bytes in current source

Build the current source to use `verify --expect-sha256 PATH=SHA256`. This option
checks the actual output against an independently supplied 64-digit SHA-256,
records a `file_hash` assertion in the Receipt, and leaves failed deliveries
unpublished. It shares the 64-requirement budget with `--expect-file` and retains
the existing sandbox. See the [output digest guide](docs/guides/output-digests.md)
for an example, limits and failure cases. This feature is not in the published
v0.2.3 archives.
