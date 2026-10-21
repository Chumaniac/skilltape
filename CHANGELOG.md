# Changelog

## Unreleased source - 2026-10-21

- Ask for a saved Skill or Receipt instead of requesting the `demo` and
  `run-a` example IDs from navigation. Keep the selected Skill in review URLs.
- Correct missing packages in place, reject path input before requesting files,
  and cancel obsolete reviews. Opening evidence does not execute a Skill.

## [0.3.1] - 2026-10-19

- Replace the Receipt slogan and oversized metric cards with a recorded-result summary, compact run details and a next review action.
- Jump directly to failed checks or execution results with keyboard focus; reset filters when opening another saved run.
- Distinguish completed runs without explicit assertions. Original Receipts, policy decisions and output fingerprints stay unchanged.

## [0.3.0] - 2026-10-19

- Package exact output digest requirements and the saved-Receipt review workbench
  for native installation, including failure filtering and recoverable errors.
- Keep all four native targets, checksums, archive/SBOM provenance, and the
  existing staged installers. Previous releases and assets remain unchanged.

## Unreleased source - 2026-10-19

- Open actual saved Receipts from the Console by filename, recover from a
  missing or unsupported document, and filter assertions needing attention.
- Read the v1 `kind` field emitted by Verify instead of the incorrect `type`
  field used by the old UI fixture; show explicit pass/fail labels and counts.
- Reject unsupported Receipt shapes before rendering, bound each displayed
  collection to 1,000 entries, and retain read-only local behavior. Existing
  v0.2.3 release assets are unchanged.

## Unreleased source - 2026-10-18

- Add repeatable `verify --expect-sha256 PATH=SHA256` requirements, using the
  existing FileHash assertion and Receipt contract. Validate unsafe paths,
  malformed digests and contradictory requirements before loading or executing
  the Skill; failed checks do not publish a delivery.
- Bound shared output hashing to 16 MiB per file and 64 MiB per verification,
  stream the bytes and reject detected replacement or changes during reading.
- Preserve v0.2.3 release assets. These capabilities require the current source.

## [0.2.3] - 2026-10-16

- Publish required-output checks, the saved-delivery workbench and comparison,
  literal Capture arguments, orderly Unix cancellation and bounded stable exports.
- Release run `38862770537` passed all four platform builds, provenance/SBOM
  generation and the public Windows installer smoke check.
- All eight downloaded archive/SBOM checksums matched. An isolated Apple Silicon
  installation passed required-file success/failure, unsafe-path rejection and
  packaged Console API/UI checks. A missing required file returned exit3 and
  did not publish a delivery directory.
- Update installation commands to v0.2.3 and its immutable installer source.
  Preserve previous tags, assets and dated source-candidate evidence below.

## Source quality — 2026-10-15

- Preserve literal command arguments through repeatable Capture `--arg` options.
- Connect Unix Replay/Verify interrupts to runner cancellation, retain cancelled
  results/Receipts with exit 5, and clean temporary execution state before exit.
- Expose caller-owned verification cancellation without changing `verify_run`.
  Cancelled runs leave output assertions unevaluated and deliveries unpublished.
- Align current installation commands with published v0.2.1; preserve historical
  demo evidence and source 0.2.3 without publishing a new release.

## Source maintenance — 2026-10-13

- Stream and validate bounded Tape event reads while retaining only the
  requested Console page, including errors outside that page.
- Bound Capture event counts and retained metadata, cancel on exhaustion, and
  move event payloads into the merge without cloning the filesystem collection.
- Finish Capture in staging before no-replace publication; failed finalization
  and destination conflicts preserve existing results.
- Preserve source 0.2.3 and existing public release assets.

## Source continuation — 2026-10-12

- Compare preserved baseline and replacement deliveries in the Console using
  actual read-only inspections and observed path, size and SHA-256 differences.
- Require complete successful evidence; reject failed, truncated, duplicate or
  malformed file summaries instead of declaring omitted files unchanged.
- Bound each input to100 files and the displayed changes to100 rows; discard
  results and cancel requests on selection, queue refresh or navigation.
- Keep source0.2.3 and public0.2.1 separate; no release or business approval.

## [0.2.3 source candidate] - 2026-10-10

- Add a visible saved-delivery Console workbench backed by actual CLI bundle
  discovery, selectable bounded pages, manual file rechecks and actionable
  integrity findings. Preserve existing Timeline/compile/Receipt views.
- Separate reported execution, physical file matching and unrun independent
  business checks. No demo IDs, Receipt copying, commands or payload contents
  are required or introduced in the delivery path.
- Add native no-follow metadata/file reads and stable hashes, exact file sets,
  capacity bounds, two concurrent blocking checks and cancelled-request cleanup.
  Reject non-loopback service binding. Native inspection currently requires POSIX;
  Windows existing read-only views and fail-closed Replay/Verify remain intact.
- Show the current Console source version from package metadata. This update is
  source-only; the public0.2.1 binaries and historical assets remain unchanged.
- Restrict detail selection to the same bounded discovered catalog as the queue.
  Enumerate layout and artifact directories through anchored handles, retain
  directory identity and keep inspection capacity occupied until cancelled work
  actually finishes.

## Source catalog continuation — 2026-10-10

- Add a locked, restricted multi-organization delivery example using native Bash
  and Perl. Combined1MiB/1,000-row input limits, ASCII identities, safe integers,
  nested tuple grouping and exclusive outputs keep the demo bounded.
- Preserve expectations and actual allocations for independent SkillSync0.1.6
  composite checks. Both correct and balanced-wrong inputs execute successfully;
  the consumer separates runtime success from requirement failures.
- Add positive, invalid-input, capacity, fixed-diagnostic and no-overwrite checks.
  Source binary0.2.2 and public0.2.1 assets remain unchanged; no release is claimed.

All notable changes to SkillTape are documented here.

## [0.2.2 source candidate] - 2026-10-09

- Bound selected export traversal/copy and package metadata reads; hash the
  completed staging files and reject detected source changes before publication.
- Share native no-replace publication across export targets. Windows directory
  publication gains a native filesystem implementation; Replay/Verify stay closed.
- Add optional `export --receipt` association with bounded, unambiguous successful
  metadata and the actual exported package digest. This is not authenticated
  execution evidence, and normal lint-only export remains available.
- Add regression coverage, source examples and a full expansion roadmap.
  Existing published v0.2.1 and all earlier successful assets remain unchanged.

## [0.2.1] - 2026-10-09

### Fixed

- Align the Windows fail-closed executor with the async command type used by
  the runner. Its existing denial test now compiles on every test host and
  asserts the exact caller type; Windows Replay/Verify still remain unavailable.
- Check production Windows binaries in ordinary CI before creating a release.
- Harden the full-SHA-pinned Rust action's toolchain/target/component inputs.

Public release run `37893425823` succeeded for all four platform builds,
provenance/SBOM/checksums and the published Windows installer. The immutable
v0.2.0 tag below remains the earlier failed Windows publication record.

## [0.2.0] - 2026-10-09

The source tag's Linux and both Mac builds passed. Its Windows build failed
with an async command type mismatch; publication and the published installer
check were skipped. No public v0.2.0 release is claimed.

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
