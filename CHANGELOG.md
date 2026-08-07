# Changelog

All notable changes to SkillTape are documented here.

## [0.1.0] - Unreleased

The implementation is merged on `main` at commit
`bdd82937fc652190917a8259098bc92ae48553cb`, and CI run `31149247700` is
green. The versioned GitHub Release remains unpublished pending final
release-matrix and Windows installer verification.

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
