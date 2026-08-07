# Release Readiness — 0.1.0 RC

Date: 2026-08-07

See the [documentation index](README.md), [installation guide](guides/installation.md),
and [release workflow](../.github/workflows/release.yml) for the surrounding
release documentation.

## Current merged-main evidence

- The implementation is merged on `main` at commit
  `bdd82937fc652190917a8259098bc92ae48553cb`.
- CI run `31149247700` is green for the merged implementation.
- The merged implementation includes interactive Capture, locked dependencies,
  installed Console discovery, release packaging, installers, smoke
  verification, and the tag-driven workflow.
- No release tag or versioned GitHub Release has been published.

## Local verification evidence

The following local checks passed for the implementation:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- `cargo test --locked --workspace -- --test-threads=1` passed.
- `npm ci`, the Console production build, and 4 Playwright tests passed.
- 4 release package tests passed, including Windows naming and symlink rejection.
- Release workflow static checks and Ruby YAML parsing passed.
- The Unix HTTPS installer fixture passed; checksum failure preserved the old CLI.
- Real release Console smoke passed for loopback API JSON and static UI HTML.

## Required before publishing

- [x] Confirm hosted Linux has bubblewrap/user namespaces and hosted macOS has
      `/usr/bin/sandbox-exec`; the current CI evidence is run `31149247700`.
- [ ] Run the [release workflow](../.github/workflows/release.yml) on all four
      matrix targets and retain only the intended archive/checksum assets:
      `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`,
      `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`.
- [ ] Execute the Windows installer fixture on Windows PowerShell and record
      the result; local macOS cannot provide this evidence.
- [ ] Review the generated `checksums.txt` and archive contents from the exact
      tag commit.
- [ ] Review [CHANGELOG](../CHANGELOG.md) and [security notes](../SECURITY.md),
      then obtain explicit approval before creating the release tag and granting
      publish permission.

## Explicit known limitations

- Windows Replay/Verify remains fail-closed because no equivalent restricted
  executor is implemented.
- The Console browser tests use mocked API responses; the real network path is
  covered by the packaged API/UI smoke script, not by Playwright against a live
  workspace.
- Historical SDD tasks 9 and 13–23 record controller fallback after stalled
  implementation/review agents. Current test results are fresh evidence, but
  they do not retroactively constitute independent review approval.
