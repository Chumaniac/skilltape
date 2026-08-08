# Contributing to SkillTape

## Public contribution workflow

- Follow the [Code of Conduct](CODE_OF_CONDUCT.md) when participating in the
  project.
- Use the [bug report form](.github/ISSUE_TEMPLATE/bug_report.yml) for
  reproducible problems and the [feature request form](.github/ISSUE_TEMPLATE/feature_request.yml)
  for proposals. Blank Issues are disabled so that public reports stay
  structured and actionable.
- Review the [pull request template](.github/pull_request_template.md) before
  opening a pull request. It covers scope, tests, documentation, and
  secret-free evidence.
- [Dependabot](.github/dependabot.yml) opens weekly Cargo, Console npm, and
  GitHub Actions update pull requests, with at most five open updates per
  ecosystem.
- Do not publish vulnerability or exploit details in an Issue or pull request;
  follow [SECURITY.md](SECURITY.md) for private disclosure.

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

## English documentation

All natural-language repository content must be English. Follow the
[documentation style guide](docs/documentation-style.md) for headings,
product vocabulary, normative wording, links, status, dates, and security
copy. Documentation changes must run the repository-wide language scan and
Markdown link audit described in that guide before opening a pull request.

## Documentation and release changes

Update the README, the relevant guide, CHANGELOG, and release-readiness evidence
when user-facing behavior or platform prerequisites change. Release archives
must be produced with `scripts/package_release.py`; do not hand-edit an archive
or publish without checksums.

The release workflow may be tested locally for syntax and fixtures, but
publishing requires an explicit GitHub repository, release tag, and permission
decision.
