# Changelog

All notable changes to SkillTape are documented here.

## [0.2.0] - 2026-10-09

### Added

- Source release candidate 0.2.0 includes the verified delivery and bounded input
  features below; public publication is verified by the new immutable tag workflow.
- Refresh locked Rust and Console dependencies and pinned workflow actions after
  compatibility and existing Console/sandbox checks.
- Lock Rustls to 0.23.45 and source-map-js to 1.2.2, the repaired versions for
  the existing GitHub dependency advisories; dependency alerts are checked
  again after the verified main update.
- Source Verify can publish a new `--delivery-dir` containing actual artifacts,
  the unchanged v1 Receipt and a checksummed manifest. Completed files survive
  assembly/publication failure in private staging. An order-summary example runs
  a local system script and is independently checked by a SkillSync contract.
- A synthetic incident-review material package with explicit file permissions,
  input-integrity assertions, and generic/Claude Code/Codex/Cursor exports.
  Local checks cover changed or missing material and an undeclared write. The
  example prepares review evidence; it does not analyze incidents or recover services.

### Fixed

- Noninteractive PTY readers no longer wait on an unrelated global stderr lock;
  output limits and truncation evidence remain unchanged.
- Each bounded input open now checks canonical containment in its selected
  inventory root; snapshots reuse the metadata obtained during root validation.
- Replay staging and Verify hashing enforce the same bounded input inventories
  and length-limited reads. Detected file growth, truncation, entry changes, and
  unsafe leaf replacements fail closed; stable inputs retain their existing digest.
- Output-directory publication now uses exclusive native rename on Linux/macOS,
  closing an empty-destination race; overlap checks resolve existing ancestors.
- Console timeline navigation now exposes all Tape and event pages, preserves
  page positions in the URL, and retains only the current page in memory.
- Captures with a zero completion timestamp now display Finished.

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
