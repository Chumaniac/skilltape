# Release Readiness

Date: 2026-10-10. Historical release evidence retains its original scope.

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
