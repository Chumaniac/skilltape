# Release Readiness

## 2026-10-29 Capture source review follow-up

CLI Compile now records normalized Capture manifest/event digests and the event
count, and Console exposes the bounded read-only summary with an explicit legacy
fallback. Old executable package files, Skill/Receipt hashes and successful
assets remain unchanged. Shared manifest reads and writes are limited to 64 KiB;
metadata changes during input reading stop publication. See the
[Tape source format](reference/tape-format.md#retained-capture-identity).

394 Rust checks passed serially with normal local process/signal permissions.
The earlier restricted-context cancellation failures are retained separately;
no cancellation implementation or assertion was weakened. A macOS hosted
starter check then exposed valid filesystem observations interleaved with a
running command. Compile now preserves command/output ordering and keeps each
command's assertions separate; it still rejects incomplete terminal groups.
The actual same retained interleaved Tape failed the old compiler and compiled
with the new one. Its shell replay remained denied by the original policy;
both separate native synthetic starters completed Capture/Compile/Verify/Export.
Production Console
and native source readback use retained synthetic Capture assets, with digests
independently recomputed and executable files byte-identical. Browser and hosted
checks are recorded by the current delivery receipt. No installed release,
deployment, paid provider or user business workflow was executed by this
follow-up; the native starter executions used new isolated synthetic inputs.

## Compile interruption guard - 2026-10-29 source

A failing synthetic test reproduced successful Compile from an explicitly
cancelled Tape. The shared compiler input now rejects recorded cancellation,
timeout and recording errors, and the CLI requires a finished manifest. Invalid
flags fail without echoing their payload. Normal finished captures and legacy
metadata-only records remain supported; no source Tape or existing output is
rewritten. Local workspace tests and Clippy passed. A real local Capture/SIGINT
flow retained the Tape, rejected Compile without output and completed a normal
Capture/Compile flow. Hosted checks and source delivery are recorded separately;
no release or installation replacement is implied.

## Starter Console discovery repair - 2026-10-22

Installed-browser acceptance found that nested starter outputs were absent from
the existing Console catalogs. Store Tapes and exact Receipt bytes in the
standard metadata directory and expose the delivery as a direct child. A
regression now passes actual starter results through the real Console read
model; generation, hash verification and previous evidence remain unchanged.

## Practical starters source candidate - 2026-10-22

Two fixed local recipes use actual Capture, deterministic Compile, strict Lint,
sandboxed Verify, output hash checks and Receipt-bound export. Scaffold creation
executes nothing. Completed and failed runs remain separate; existing outputs
are never overwritten. The generic export omits private inputs. The compiled
producer points to the selected installed binary, which must remain available.

Focused tests cover both complete flows, mismatched output requirements, missing
and changed inputs, links, traversal, size limits, duplicate destinations and
explicit execution consent. Full workspace checks, hosted main checks and the
installed-package browser flow are tracked separately. No new remote Release
or deployment is implied by this source candidate.

## Version 0.4.1 terminal metadata repair

An invalid synthetic step label reproduced raw terminal control bytes in human preflight output. Render labels through Rust string debug escaping; retain the label as data and preserve JSON output. Negative CLI cases cover ESC, newline and a directional control. Keep v0.4.0 source/tag/assets as history; the patch uses a new immutable release.

## Version 0.4.0 distribution candidate

Adds read-only CLI and Console policy preflight backed by the replay policy checks. The preview is limited to 1,000 steps/checks and the UI renders 32 checks. It is not an execution or input-acceptance result. Required native and Console CI, release archives and an installed-package flow remain separate gates. Existing release and successful Receipt assets are retained.


## Version 0.3.1 distribution candidate

A frontend-only review workflow change plus matching CLI/API/UI package versions. Native main and release gates remain required. Baseline screenshot and failing old-version next-action check were recorded before implementation; the new browser flow verifies focus, failure filtering and selection reset. Preserve previous releases and existing successful artifacts.


Date: 2026-10-19. Historical release evidence retains its original scope.

## Version 0.3.0 distribution candidate

This candidate packages the output-digest CLI and saved-Receipt workbench on
all four existing native targets. CLI/API/Console metadata are aligned to
0.3.0. Main CI must pass before creating the immutable tag; the Release
workflow then supplies archive checksums, SBOM/provenance, packaged Console
checks and the published Windows installer gate. Installed-package journey
evidence is recorded separately after downloading the published assets.


## Console Receipt review source candidate - 2026-10-19

The Console now opens saved Receipts by filename, shows explicit assertion
outcomes and counts, and filters failed checks without hiding the overall run
status. The previous fixture used `type` while the real v1 schema emits `kind`;
using the actual field reproduced a missing Receipt page before the repair.
Unsupported documents fail visibly rather than crashing or becoming accepted
evidence. This is a source update; no release or deployment is performed.

Local validation passed the Console build and 19 browser checks, including
canonical fields, failure filtering, missing-file recovery, path rejection,
cross-Receipt state reset, literal untrusted text and mobile layout. A native
CLI generated matching and mismatched digest Receipts (exit 0 and 3); the real
local API and built UI opened both, selected the failed hash check and produced
desktop/mobile captures without a mocked API. Existing successful Receipts
were reused during the harness connection retry.

The unchanged Rust workspace, warning-free Clippy, formatting, package/workflow
fixtures and installer checks passed. Review rendering is bounded to 1,000
entries per collection and 4,096 characters per text field; CPU/RSS were not
measured. Hosted checks remain required before merging this candidate.

## Output digest source candidate - 2026-10-18

The current source exposes `verify --expect-sha256 PATH=SHA256` through the
existing assertion and Receipt model. Caller-supplied hashes are checked before
execution; shared hashing is bounded to 16 MiB per file and 64 MiB per Verify.
This candidate retains the executor, permission policy, no-overwrite behavior,
and v0.2.3 public assets. It does not publish a new package or release.

Regression evidence covers a correct reference, changed bytes, invalid and
unsafe requirements, combined capacity, contradictory aliases, failed delivery
non-publication, file size limits, shared read budgets and replacement detection.
Required hosted checks remain the merge gate; source validation is separate
from release installation and business acceptance.

Local validation passed 340 Rust tests, warning-free workspace Clippy and
formatting, the Console production build, 14 browser checks using an isolated
installed-Chrome profile, 4 release-package fixtures, 14 workflow fixtures and
the installer fixture. The pinned Playwright browser was absent, and nested
macOS isolation was unavailable in the desktop sandbox; the unchanged tests
passed with native isolation and the installed Chrome. Language, local links
and whitespace checks passed. Existing workspace `libc` is reused without a
new dependency package or version upgrade. CPU and process RSS were not measured.

## Public v0.2.3 — 2026-10-16

Release run `38862770537` passed four native platform builds, archive/SBOM
provenance, release publication and the published Windows installer check.
The tag points to verified main commit
`9f6bec579258baa63234139af0fae650d9efdbd2`. All eight downloaded archive/SBOM
checksums matched, and native archive attestation verification passed.

An isolated Apple Silicon installation from the public release passed packaged
Console API/UI checks and actual required-output verification. An existing
regular file passed with exit0; a missing file returned assertion_failed/exit3
and left its delivery unpublished. An unsafe relative path returned exit2.
This is synthetic local release acceptance, not customer or business approval.
Windows isolated Replay/Verify remain unsupported.


## Source quality — 2026-10-15

Current source passes literal Capture argv without shell parsing and wires Unix
Replay/Verify interrupts through caller-owned cancellation. Cancelled verification
returns exit 5, retains a requested cancelled Receipt, skips output assertions,
and leaves incomplete deliveries unpublished. Regression fixtures cover literal
spaces/metacharacters, missing command, running-process interrupts, cleanup,
Receipt persistence, delivery non-publication and pre-cancelled empty workflows.
The previous binary exited on SIGINT without JSON or a Receipt in a synthetic
one-second wait; current regression fixtures assert orderly cancellation.

Current installation guides now select verified public v0.2.1 and its immutable
installer commit. The historical terminal recording and release evidence remain
unchanged. This slice does not publish a binary or change Windows isolation.

Local macOS validation passed: 329 locked Rust tests, formatting, warning-free
Clippy, Console production build, 14 Console tests using installed Chrome,
release-package/workflow/documentation fixtures, installer fixtures and the
Capture-to-Receipt quickstart. The pinned Playwright browser was absent locally;
the same browser assertions passed in a separate Chrome test profile. The
workflow fixture used an existing Python environment with PyYAML. Repository
language and prose-link checks passed. Hosted required checks remain the merge
gate; local evidence does not establish release or production acceptance.

## Source maintenance — 2026-10-13

The current maintenance slice bounds Console Tape reads and Capture retention,
preserves complete-stream validation during pagination, and finishes new Tapes
before no-replace publication. Regression fixtures cover byte/event limits,
growth after opening, page-external errors, allocated string capacity,
cancellation signaling, failed finalization and destination preservation.
Validation applies to local synthetic data and source checks; it does not
publish binaries, run customer workflows or establish production acceptance.

Initial local macOS validation passed: 321 locked Rust tests, formatting, warning-free
Clippy, Console production build, 14 Console checks in installed Chrome,
release-package/workflow fixtures, installer fixtures and documentation checks.
The pinned Playwright browser was absent locally, so the same Console assertions
ran with a separate installed-Chrome test profile. Remote required checks remain
the merge gate. The documentation link checker now excludes fenced historical
examples while continuing to check actual relative navigation links.

See the [documentation index](README.md), [installation guide](guides/installation.md),
and [release workflow](../.github/workflows/release.yml) for the surrounding
release documentation.

## Source candidate 0.2.2 — 2026-10-09

Continuation on2026-10-10 adds catalog data/scripts rather than a new binary
version. The native macOS synthetic tenant producer passed three focused tests;
separate preserved good/wrong deliveries were independently consumed by
SkillSync source0.1.6. Both execution Receipts succeeded and bound actual files;
the composite consumer accepted the good set and rejected two wrong allocations.
A single local-ID control accepted both balanced totals. Full source CI is
checked for the continuation commit separately; no new release assets are claimed.

The current source adds bounded staged-byte export, shared no-replace
publication and optional successful Receipt association. These capabilities
are absent from the existing v0.2.1 release assets; no new release is claimed.

- Full locked Rust workspace tests, formatting and warning-free Clippy passed
  on macOS; package (4), workflow (14), documentation (10) and installer
  fixture checks passed. Console build and all 5 existing browser tests passed
  on installed Google Chrome with mocked API responses.
- A separate native macOS synthetic Capture → Compile → Lint → Verify →
  Receipt-linked Export passed. Independently hashing the actual export bytes
  reproduced both the manifest fingerprint and the successful Receipt hash.
  Existing successful stages were retained and reused.
- New regressions cover capacity, stable copying, no-overwrite publication,
  bounded metadata and invalid/mismatched Receipt association. Independent
  read-only code review found no blocking issue; this is not expert security
  approval or real Agent acceptance.
- The added Windows job compiles and tests export/publication only. Its actual
  hosted result must be checked for the current commit before claiming Windows
  runtime evidence. Replay/Verify remains unsupported on that platform.
- Main CI, a separately authorized tag, four-platform release assets and
  installed-release journeys remain distinct gates. No signing or trusted
  execution identity is supplied by the optional Receipt association.
- The repository language scan found no natural-language matches. The raw
  Markdown audit retains 8 known historical fenced-code example targets;
  all rendered prose links and new document links resolve.

## Source candidate 0.2.3 — 2026-10-10

The new workbench consumes actual retained CLI delivery folders independently
of the legacy mocked run/Receipt registry. It reports physical file observations,
saved execution metadata and separate requirement/authentication boundaries.
Native API regression checks and the extended local browser suite are recorded
in the current source delivery evidence. Full Rust/Console, packaging/installer
and current CI checks remain required before main; source is not a new release.
See the [saved delivery contract](reference/saved-deliveries.md).

The local candidate passes311 locked workspace tests,21 native API regression
cases, format/Clippy and actual desktop/mobile Console journeys. Discovery and
physical inspection retain one bounded work slot across both stages; metadata,
layout and inventory use actual catalog entries and directory handles. Current
exact-head hosted checks must still pass; local evidence is not a release.

## Historical 0.1.0 merged-main evidence

- The implementation and release workflow are merged on `main` at commit
  `beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6`.
- Final tag-triggered release run `31167200699` is green for the four target
  builds, publication, and the Windows installer smoke job.
- The merged implementation includes interactive Capture, locked dependencies,
  installed Console discovery, release packaging, installers, smoke
  verification, the tag-driven workflow, and a Windows PowerShell installer
  smoke job for published release assets.
- The `v0.1.0` tag points to the release commit above, and the [published
  GitHub Release](https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0)
  contains the four target archives and `checksums.txt`. It was published
  before archive SPDX SBOM and GitHub provenance/SBOM attestation generation
  was added, so it remains a historical checksum-only release.

## Future-release integrity contract

Future release runs must begin from an existing matching `v<version>` tag
whose commit resolves to the workflow commit. For every target archive, the
workflow must:

- generate an archive-local SPDX JSON sidecar named `<archive>.spdx.json`;
- create GitHub artifact attestations for the archive's build provenance and
  its SPDX SBOM predicate; and
- publish SHA-256 entries for the archive and sidecar in `checksums.txt`.

These requirements apply to future releases only. They do not add provenance
or an SBOM retroactively to `v0.1.0`.

## Local verification evidence

The following local checks passed for the implementation:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- `cargo test --locked --workspace -- --test-threads=1` passed.
- `npm ci`, the Console production build, and 4 Playwright tests passed.
- 4 release package tests passed, including Windows naming and symlink rejection.
- Release workflow static checks and Ruby YAML parsing passed.
- The Unix HTTPS installer fixture passed with `./` checksum paths; checksum
  failure preserved the old CLI.
- The Windows installer now supports authenticated GitHub release API asset
  downloads and normalizes checksum filenames from the published manifest.
- Real release Console smoke passed for loopback API JSON and static UI HTML.

## Completed release gates

- [x] Confirm hosted Linux has bubblewrap/user namespaces and hosted macOS has
      `/usr/bin/sandbox-exec`; the current CI evidence is run `31149247700`.
- [x] Run the [release workflow](../.github/workflows/release.yml) on all four
      matrix targets and retain only the intended archive/checksum assets:
      `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`,
      `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`.
- [x] Execute and record the Windows PowerShell installer smoke job against
      the published release assets.
- [x] Review the generated `checksums.txt` and archive contents from the exact
      tag commit.
- [x] Review [CHANGELOG](../CHANGELOG.md) and [security notes](../SECURITY.md),
      then publish the approved `v0.1.0` release.

## Post-release follow-up

### Source Console pagination — 2026-10-04

- Added previous/next navigation for 50-Tape and 100-event pages, with URL
  restoration, failed-page retry, and bounded current-page rendering.
- Reproduced the missing navigation and zero-timestamp status defects before
  changing the implementation. The Console production build and all 5
  Playwright tests passed after the change, including 51 synthetic Tapes and
  101 synthetic events. Desktop and 390px mobile screenshots were reviewed.
- Browser responses were mocked. No real capture, replay, Rust workspace
  tests, API smoke, or release packaging was run for this UI-only change.
  The published v0.1.0 assets remain unchanged.
- The language scan and whitespace check passed. The raw repository Markdown
  link audit reported 8 existing example links inside fenced code blocks in
  the historical user-first documentation plan; rendered prose links passed
  a fence-aware audit. No historical example content was changed.

- Update the third-party GitHub Actions that currently emit Node.js 20
  deprecation warnings before the next maintenance release. This is a
  non-blocking warning for v0.1.0 because the final release run completed
  successfully.

## Explicit known limitations

- Windows Replay/Verify remains fail-closed because no equivalent restricted
  executor is implemented.
- The Console browser tests use mocked API responses; the real network path is
  covered by the packaged API/UI smoke script, not by Playwright against a live
  workspace.
- Historical SDD tasks 9 and 13–23 record controller fallback after stalled
  implementation/review agents. Current test results are fresh evidence, but
  they do not retroactively constitute independent review approval.
