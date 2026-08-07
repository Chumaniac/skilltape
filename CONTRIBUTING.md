# Contributing to SkillTape

## Development prerequisites

- Rust 1.97.1; the repository `rust-toolchain.toml` selects the pinned toolchain.
- Node.js 22 for Console; use `npm ci --prefix apps/skilltape-console`.
- Linux Replay/Verify development requires `bubblewrap`; macOS requires
  `/usr/bin/sandbox-exec`. Windows intentionally fails closed for those commands.

## Local checks

Run the same gates used by CI before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
npm --prefix apps/skilltape-console test
python3 scripts/test_release_package.py
python3 scripts/test_release_workflow.py
bash scripts/test_install.sh
```

For changes to Capture, the runner, policy, Receipts, exporters, or Console
process boundaries, add a regression test first. Do not weaken an assertion or
skip a security gate to make an implementation pass.

## Scope and security

- Keep changes focused on the requested behavior; defer unrelated cleanup.
- Never commit API keys, tokens, cookies, environment files, Tape contents,
  Receipt contents, or raw command output.
- Preserve schema versioning, workspace-relative path rules, redaction, and
  no-overwrite behavior.
- Treat controller fallback reports as implementation history, not independent
  review approval; request a fresh review for security-sensitive changes.
- Use `SECURITY.md` for vulnerability reports rather than public exploit details.

## Documentation and release changes

Update the README, the relevant guide, CHANGELOG, and release-readiness evidence
when user-facing behavior or platform prerequisites change. Release archives
must be produced with `scripts/package_release.py`; do not hand-edit an archive
or publish without checksums.

The release workflow may be tested locally for syntax and fixtures, but
publishing requires an explicit GitHub repository, release tag, and permission
decision.
