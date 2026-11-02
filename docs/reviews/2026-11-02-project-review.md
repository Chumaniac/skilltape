# SkillTape project status and optimization review

Status: reviewed source and follow-up plan. Date: 2026-11-02, Asia/Shanghai.

Reviewed baseline: `69d1828e9a18a6b50fe25405fd2060036331f238`.
The implementation references below apply to that baseline. This change adds a
review document; the proposed implementation changes remain follow-up work.

## Current stage

SkillTape has a substantial local Capture, Compile, Verify, Receipt, and Export
workflow. Current source includes features beyond the unchanged v0.4.1 download
archives. Source completion, published binaries, platform support, and business
acceptance therefore require separate statements.

## Implemented capabilities

- Capture, Compile, Lint, Verify, Receipt, Export, policy preflight, Console,
  Starter, and recoverable Batch have implementation paths. See
  [the source/release distinction](../../README.md#L14) and
  [Batch commands](../../crates/skilltape-cli/src/batch_command.rs#L730).
- Batch prepare does not execute a case. Pending execution, explicit retries,
  cancellation, and unconfirmed outcomes have state constraints. See
  [Batch state](../../crates/skilltape-core/src/batch.rs#L182).
- Input files, process stdout/stderr, Console pages, Receipt/context reads, and
  retained Batch material have several existing capacity limits. See
  [workspace limits](../../crates/skilltape-runner/src/workspace.rs#L11),
  [process defaults](../../crates/skilltape-runner/src/lib.rs#L65), and
  [Batch admission](../../crates/skilltape-cli/src/batch_command.rs#L22).

## Completion boundaries

- Starter and Batch belong to current source and are not included in the
  unchanged v0.4.1 archives. Windows Replay and Verify remain fail-closed. See
  [the README](../../README.md#L14) and
  [Windows support](../../README.md#L90).
- A Receipt records bounded local execution and file relationships. It does not
  prove an authenticated executor, business semantics, remote delivery, or
  long-term CPU/RSS performance. See
  [release-readiness limits](../../docs/release-readiness.md#L114).

## Prioritized follow-up

P1 identifies a correctness or delivery boundary to resolve first. P2 covers
reliability and maintenance. These priorities are not vulnerability ratings.

### P1 Apply one budget to generated files and copied scripts

Evidence: a source-level capacity gap; no oversized execution was run. Input
budgets and process-output limits do not bound all generated files. The
`file_hash`, materialization, and referenced-script copy paths do not share a
symmetric count and byte budget.

Acceptance: share file-count, depth, single-file, and total-byte limits across
hashing, materialization, and the delivery manifest. Growing, replaced, or
oversized outputs must fail without publishing a partial delivery.

References: [assertion execution](../../crates/skilltape-runner/src/execution.rs#L212),
[output materialization](../../crates/skilltape-runner/src/workspace.rs#L85),
[script copying](../../crates/skilltape-runner/src/workspace.rs#L413), and
[Workflow arrays](../../schemas/workflow/v1.json#L39).

### P2 Align resource validation with run-context binding

Evidence: source constraints differ. Runner `validate_limits` does not reject
`max_output_bytes: 0`, while context binding requires a positive value. A library
caller could obtain a run whose sidecar cannot bind to its Receipt.

Acceptance: reject invalid and unsupported limits at entry, then cover execution,
save, Console reading, and Batch binding with the same accepted values.

References: [runner validation](../../crates/skilltape-runner/src/lib.rs#L463),
[context binding](../../crates/skilltape-verify/src/context.rs#L91).

### P2 Bound and validate Console diff and run inputs

Evidence: source-level maintenance and resource risk. Diff validation checks
structure without equivalent collection limits; the run read model parses JSON
without validating the complete run v1 contract.

Acceptance: validate run v1 on the server, bound array counts and field lengths,
and paginate large collections. Malformed or oversized fixtures must not reach
unbounded page rendering.

References: [client validation](../../apps/skilltape-console/src/api.ts#L158),
[run read model](../../apps/skilltape-console-api/src/read_model.rs#L504),
[review rendering](../../apps/skilltape-console/src/pages/CompileReview.tsx#L98).

### P2 Commit or roll back all installer components together

Evidence: source-level failure-path risk; installation failure was not injected.
If replacing Console fails after CLI/API replacement, the current recovery path
can restore only Console and leave mixed component versions.

Acceptance: retain and atomically commit or restore the previous CLI, API, and UI
as one installation. Inject failure in the last replacement step and verify
component versions and existing user data remain intact.

Reference: [Unix installer](../../scripts/install.sh#L166).

## Verification record

- The reviewed baseline had nine successful hosted checks covering the existing
  platform, Rust, Console, and CodeQL jobs.
- The ten existing user-documentation checks passed locally during this review.
- This review did not run the full Rust suite locally, installer failure
  injection, Windows Replay, a real user workflow, or a new release.
- The repository-wide language scan, Markdown link audit, and whitespace check
  apply to this document revision. Existing protected-main checks remain in
  force; this document does not change their assertions or configuration.
