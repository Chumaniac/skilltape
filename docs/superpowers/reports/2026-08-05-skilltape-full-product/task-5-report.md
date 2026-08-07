# Task 5 Report

## Result

- Added `watch_workspace(root, tx, CancellationToken)` and documented public filesystem change/error types.
- Uses `notify::PollWatcher` with a bounded 50 ms interval and content comparison so temporary workspaces behave consistently on the current macOS runner.
- Emits deterministic workspace-relative `/`-normalized `Created`, `Modified`, `Moved`, and `Deleted` changes.
- Rejects lexical and canonical root escapes, including symlinks resolving outside the root.
- Records streaming SHA-256 and size metadata without retaining or persisting file contents.
- Coalesces duplicate raw events, suppresses redundant create/modify pairs, and sorts each batch by normalized path.
- Uses native rename pairs when supplied. For PollWatcher, exactly one distinct create/remove pair in a batch is inferred as `Moved`; ambiguous multi-pair batches remain separate events.
- Cancellation is observed while idle, batching, or blocked on the bounded output channel, and the watcher is dropped before return.

## Failure, root cause, and fix

1. Controller reproduction before the final watcher change:

       cargo test -p skilltape-capture --test filesystem_capture

   Output: 5 tests ran; root rejection passed and four watcher tests timed out at `tests/filesystem_capture.rs:66` while waiting for the readiness `Created` event.

2. Initial diagnostics selected between the event receiver and watcher task and logged the raw callback boundary. The watcher task did not exit with an error in local runs. Local FSEvents runs produced `Create(File)` for the marker, but the controller failure remained reproducible outside that run.

3. Controller standalone `notify 8` probe:

   - `recommended_watcher` (macOS FSEvents) emitted no event for a temporary directory even after `watch()` plus 500 ms.
   - `PollWatcher` with `Config::default().with_poll_interval(Duration::from_millis(50))` immediately emitted `Ok(Event { kind: Create(Any), paths: [...] })`.

   Root cause: the recommended macOS FSEvents backend accepted the temporary-root watch but did not deliver file-level events on this runner. The failure was not a path-normalization error or hidden watcher-task error.

4. Minimal production fix: replace `recommended_watcher` with `PollWatcher`, use a documented 50 ms poll interval, and enable `with_compare_contents(true)`. Content comparison is required because the default polling snapshot missed rapid rewrites when filesystem write timestamps had not advanced, including the same-length `before` to `after` test.

5. PollWatcher rename evidence was `Create(Any)` for the destination plus `Remove(Any)` for the source in one poll batch, with no tracker. The adapter now infers `Moved` only for one unambiguous distinct pair.

6. Tests use an exact first-poll one-shot barrier, bounded receive timeouts, and observed marker events. They contain no arbitrary sleeps. If setup exits early, the typed watcher task result is reported immediately.

## Commands and output

- Baseline:

       PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo test -p skilltape-capture

  Output: 14 passed, 0 failed.

- TDD RED:

       PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo test -p skilltape-capture --test filesystem_capture

  Output: compile failed with unresolved imports for `watch_workspace`, `FilesystemCaptureError`, `FilesystemChange`, and `FilesystemChangeKind`.

- Final focused stability verification:

       PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo test -p skilltape-capture --test filesystem_capture

  Output: 5 passed, 0 failed. The same command was then repeated four more times with `--quiet`; every run reported 5 passed, 0 failed (25/25 total across five consecutive runs).

- Final workspace verification:

       PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo test --workspace

  Output: 83 passed, 0 failed across unit, integration, and doc-test targets.

- Final lint verification:

       PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo clippy -p skilltape-capture --all-targets -- -D warnings

  Output: exit 0; `Finished dev profile`, no warnings.

- Final formatting verification:

       PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo fmt --all -- --check

  Output: exit 0, no output.

- One combined format/test invocation accidentally allowed the second command to use the broken Homebrew Rust shim. It aborted before tests because `/Users/chumanic/homebrew/brew/opt/llvm/lib/libLLVM.dylib` is missing. All recorded verification results above used the explicit rustup toolchain and completed successfully.

## Concerns

- Polling every 50 ms with content comparison reads workspace files repeatedly and can be expensive for large workspaces. It is the deterministic backend selected for the current macOS CI behavior; future tuning should be measured rather than weakening event guarantees.
- PollWatcher has no native rename tracker. One create/remove pair in the same batch is safely treated as `Moved`; multiple simultaneous pairs are intentionally left as separate `Created`/`Deleted` changes instead of guessing the pairing.
- `watch_workspace` provides deterministic filesystem-channel ordering. Cross-source PTY/filesystem time-window merging remains a caller-level integration because Task 4 exposes no combined event-stream API in this crate.
- `Cargo.lock` remains untracked and will not be staged, as requested. Pre-existing Task 2 and Task 4 report edits are also excluded from this commit.

## Fix round 2

- Added the public `merge_capture_timeline` API with timestamped filesystem changes, tape events, configurable time-window batching, and deterministic timestamp/source/event-key ordering.
- Changed `RenameMode::Any` to infer `Moved` only for exactly one existing destination and one missing source; ambiguous batches now emit conservative `Created`/`Deleted` changes without positional pairing.
- Treated a `NotFound` race between metadata and `File::open` as missing metadata/hash while preserving other inspection errors.
- Replaced the unbounded raw notify queue with a capacity-64 queue. Overflow is surfaced as a typed `RawEventOverflow` error and is observed while idle, batching, hashing, or blocked on public output; cancellation remains selectable in each state.
- Added focused coverage for merge/window/tie and mixed-kind ordering, ambiguous rename batches, lexical escape, disappearance/open races, blocked output cancellation, batching cancellation, and raw queue overflow.

### Fix-round verification

- Explicit rustup `cargo test -p skilltape-capture --test filesystem_capture --quiet`: 9 passed, 0 failed.
- Explicit rustup `cargo test --workspace --quiet` with isolated `CARGO_TARGET_DIR=/private/tmp/skilltape-task5-target`: all workspace targets passed.
- Direct rustup `rustfmt --edition 2021 --check` on changed Rust files: passed.
- Explicit rustup `cargo clippy -p skilltape-capture --all-targets -- -D warnings`: passed with no warnings.

### Residual concern

- The merge API intentionally receives filesystem timestamps from its caller because `watch_workspace` remains backward-compatible; integrating timestamp assignment into a future tape writer is outside this fix round.
- Raw queue overflow aborts capture with an explicit error rather than dropping events, preserving correctness at the cost of requiring the caller to retry or report the incomplete capture.

## Security correction

- `normalize_workspace_path` now canonicalizes the nearest existing ancestor before checking canonical root containment, rejecting nonexistent descendants reached through outside symlinks while preserving lexical traversal checks and legitimate new paths under the root.
- Added a Unix regression test for an outside symlink with a nonexistent descendant.
