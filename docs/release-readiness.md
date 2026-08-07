# Release Readiness — 0.1.0 RC

Date: 2026-08-07

## Verified in the isolated worktree

- Branch: `codex/skilltape-foundation`.
- Latest implementation commits include interactive Capture, locked
  dependencies, installed Console discovery, release packaging, installers,
  smoke verification, and the tag-driven workflow.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- `cargo test --locked --workspace -- --test-threads=1` passed.
- `npm ci`, Console production build, and 4 Playwright tests passed.
- 4 release package tests passed, including Windows naming and symlink rejection.
- Release workflow static checks and Ruby YAML parsing passed.
- Unix HTTPS installer fixture passed; checksum failure preserved the old CLI.
- Real release Console smoke passed for loopback API JSON and static UI HTML.
- GitHub remote `Chumaniac/skilltape` is configured and the implementation is
  pushed on `codex/skilltape-foundation` with Draft PR #1.
- Hosted CI passed on both push and pull request runs for Linux/macOS Rust and
  Linux/macOS Console: runs `31146656434` and `31146658048`.

## Required before publishing

- [x] Configure and verify the authoritative GitHub remote and repository owner.
- [ ] Run the release workflow on all four matrix targets and retain only the
      intended archive/checksum assets.
- [x] Confirm hosted Linux has bubblewrap/user namespaces and hosted macOS has
      `/usr/bin/sandbox-exec`; record the actual CI run IDs above.
- [ ] Execute the Windows installer fixture on Windows PowerShell and record
      the result; local macOS cannot provide this evidence.
- [ ] Review the generated `checksums.txt` and archive contents from the exact
      tag commit.
- [ ] Review CHANGELOG and security notes, then create the release tag only
      after the remote, version, and publish permission are confirmed.

## Explicit known limitations

- The implementation branch and Draft PR are pushed, but no release tag or
  GitHub Release has been published yet.
- Windows Replay/Verify remains fail-closed because no equivalent restricted
  executor is implemented.
- The Console browser tests use mocked API responses; the real network path is
  covered by the packaged API/UI smoke script, not by Playwright against a
  live workspace.
- Historical SDD tasks 9 and 13–23 record controller fallback after stalled
  implementation/review agents. Current test results are fresh evidence, but
  they do not retroactively constitute independent review approval.
