# Release Readiness — 0.1.0

Date: 2026-08-07

See the [documentation index](README.md), [installation guide](guides/installation.md),
and [release workflow](../.github/workflows/release.yml) for the surrounding
release documentation.

## Current merged-main evidence

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

Future release runs must begin from an existing protected `v<version>` tag
whose commit matches the workflow commit. For every target archive, the
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
