# Changelog

All notable changes to SkillTape are documented here.

## [0.1.0] - 2026-08-07

The implementation and release workflow are merged on `main` at commit
`beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6`. Final release run `31167200699`
passed all four target builds, release publication, and the Windows
PowerShell installer smoke test. The published assets are available from the
[SkillTape v0.1.0 GitHub Release](https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0).

### Added

- Local-first Capture → Tape → Compile → Lint → Replay/Verify → Receipt → Export flow.
- Redacted PTY capture with interactive stdin forwarding and unique Tape IDs.
- Deterministic generic and Claude Code exporters with plugin contract validation.
- Read-only local Console API, React UI, CLI supervisor, and packaged Console assets.
- Locked Rust/npm dependencies, Linux/macOS CI gates, release archive packaging,
  checksum verification, and Unix/Windows installers.

### Security and compatibility

- Replay/Verify require bubblewrap on Linux or sandbox-exec on macOS.
- Windows supports Capture, Compile, Lint, and Export; Replay/Verify fail closed
  until an equivalent sandbox is implemented.
- Capture and Console reject unsafe path/symlink boundaries and persist redacted
  or digest-only evidence.
