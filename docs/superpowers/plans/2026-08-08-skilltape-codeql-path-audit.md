# SkillTape CodeQL path-safety audit implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the current 65 CodeQL path-injection findings through evidence-backed test hygiene, boundary regression coverage, an individual alert ledger, and narrow GitHub dispositions without changing intentional local-path behavior.

**Architecture:** Treat the audit as a boundary proof rather than a global static-analysis exception. First remove avoidable test-fixture sources, then add targeted evidence for the Console's canonical-root and symbolic-link controls. Record all 65 alerts individually, keep every verified package/replay/Console safeguard intact, and defer GitHub dismissal until the ledger and a matching CodeQL scan support the exact action.

**Tech Stack:** Rust 2021 workspace, `tempfile`, Axum/Tokio route tests, Python `unittest`, GitHub CodeQL default setup, GitHub CLI, Markdown.

## Global constraints

- Work only in the isolated `.worktrees/skilltape-codeql-path-audit` checkout on `codex/skilltape-codeql-path-audit`; do not modify `main` or the preserved `skilltape-foundation` worktree.
- Account for all 65 alerts from CodeQL analysis SHA `7366106f4af752c5a9df9f18a8e10eec245b9898`.
- Do not add a global CodeQL exclusion, source-code suppression, query override, threat-model downgrade, bulk dismissal, or unrelated refactor.
- Preserve intentional explicit local CLI paths and the current fail-closed Windows Replay/Verify behavior.
- A demonstrated untrusted package, Console request, or CI data path that escapes its intended root is a blocker: stop, update the approved design, add a failing regression test, and make only the bounded production fix that test requires.
- Test-only refactors and evidence-only tests must not alter production source behavior. Any production change follows the red-green-refactor cycle in `superpowers:test-driven-development`.
- All added prose is English, uses sentence-case headings, contains no machine-specific absolute path, and follows `docs/documentation-style.md`.
- Never include credentials, environment contents, or user-specific filesystem locations in commits, CodeQL dismissal comments, or reports.
- Do not bypass required review or merge the protected `main` branch. Branch publication and any GitHub alert mutation occur only after the local checks and ledger are complete.

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/skilltape-core/Cargo.toml` | Adds the existing workspace `tempfile` crate as a test-only dependency. |
| `crates/skilltape-core/tests/package_validation.rs` | Uses an owned `TempDir` fixture instead of a predictable environment-derived temporary path. |
| `crates/skilltape-core/src/template.rs` | Uses `tempfile::TempDir` inside its existing `#[cfg(test)]` module; production template creation remains unchanged. |
| `crates/skilltape-compiler/Cargo.toml` | Adds the existing workspace `tempfile` crate as a test-only dependency. |
| `crates/skilltape-compiler/tests/deterministic_compile.rs` | Uses an owned `TempDir` in the generated-package test and removes the hand-built temporary path helper. |
| `Cargo.lock` | Records the two test-only package dependency edges required for locked Cargo verification; no dependency version change is expected. |
| `apps/skilltape-console-api/tests/routes.rs` | Proves that collection endpoints ignore symbolic-link entries and reject cross-platform unsafe route IDs. |
| `docs/security/codeql-path-audit.md` | Contains the public, per-alert source/sink/control/decision ledger. |
| `docs/README.md` | Links contributors to the CodeQL path-safety audit record. |
| `docs/superpowers/plans/2026-08-08-skilltape-codeql-path-audit.md` | This execution plan. |

## Alert decision map

The ledger must contain exactly one row for each alert. The final state is determined only after the matching scan completes.

| Alert IDs | Initial classification | Required evidence | Expected final disposition if the alert remains on the scanned revision |
| --- | --- | --- | --- |
| 1-7 | Trusted local release-tool input | `safe_component`, regular-file/directory checks, symlink rejection, and the fixed release workflow arguments | `won't fix` individually |
| 8-9 | Console directory entry below canonical root | `storage_child`, `ensure_safe_path`, symbolic-link filtering, and Task 2 route coverage | `false positive` individually |
| 10-16, 55-59, 62, 64-65 | Explicit local Capture/Tape paths | local CLI contract, default-output canonical confinement, no-overwrite behavior, and existing Capture tests | `won't fix` individually |
| 17-46, 63 | Test fixture source | Task 1 removes the hand-built `std::env::temp_dir()` source and the focused test suites remain green | fixed by scan; otherwise `used in tests` individually |
| 47-48, 50-54 | Package-derived Replay path after validation | `validate_relative_path`, `resolve_under`, `ensure_no_symlink_ancestors`, and existing package/replay security tests | `false positive` individually |
| 49 | Test-only fake adapter read | static relative names in the test adapter and the test target classification | `used in tests` individually |
| 60 | Canonicalized local Capture workspace metadata check | `resolve_workspace` canonicalization and `validate_options` directory check | `false positive` individually |
| 61 | Explicit local Console executable override metadata check | `SKILLTAPE_CONSOLE_API_BIN` is a documented local override and `validate_api_binary` only checks file metadata | `false positive` individually |

During ledger creation, expand the decision-map ranges into the exact individual alert numbers `1` through `65`; do not combine multiple numbers into one ledger row.

### Task 1: Remove hand-built temporary paths from test fixtures

**Files:**

- Modify: `crates/skilltape-core/Cargo.toml`
- Modify: `crates/skilltape-core/tests/package_validation.rs`
- Modify: `crates/skilltape-core/src/template.rs`
- Modify: `crates/skilltape-compiler/Cargo.toml`
- Modify: `crates/skilltape-compiler/tests/deterministic_compile.rs`
- Modify: `Cargo.lock`

**Interfaces:**

- Consumes: workspace dependency `tempfile = "3"` and the existing `TempDir` cleanup-on-drop contract.
- Produces: test fixtures that own their temporary roots for the full test lifetime and no longer call `std::env::temp_dir()` to construct predictable names.
- Does not change: `create_skill_template`, `SkillPackage`, `DeterministicCompiler`, any production API, or test assertions about package behavior.

- [ ] **Step 1: Verify the focused suites on the current fixture implementation**

Run:

```bash
cargo test --locked -p skilltape-core --test package_validation
cargo test --locked -p skilltape-core template::tests
cargo test --locked -p skilltape-compiler --test deterministic_compile
```

Expected: all three commands pass before the refactor. This task changes test infrastructure only, so a passing baseline is the required behavioral reference rather than a red production test.

- [ ] **Step 2: Add the existing workspace dependency only where test code needs it**

Add this test-only dependency to both `crates/skilltape-core/Cargo.toml` and `crates/skilltape-compiler/Cargo.toml`:

```toml
[dev-dependencies]
tempfile.workspace = true
```

Keep existing `skilltape-core = { path = "../skilltape-core" }` under the compiler's same `[dev-dependencies]` table. Do not add `tempfile` to production dependency tables.

- [ ] **Step 3: Replace the package-validation fixture with an owned temporary root**

In `crates/skilltape-core/tests/package_validation.rs`, replace the static counter and `Drop` implementation with a `TempDir` owned by `TestPackage`:

```rust
use tempfile::TempDir;

struct TestPackage {
    _temp: TempDir,
    root: PathBuf,
}

impl TestPackage {
    fn valid() -> Self {
        let temp = TempDir::new().expect("temporary package parent");
        let root = temp.path().join("package");
        fs::create_dir(&root).expect("temporary package root should be created");
        let package = Self { _temp: temp, root };
        // Keep the existing six fixture writes unchanged.
        package
    }

    fn sibling(&self, name: &str) -> PathBuf {
        self._temp.path().join(name)
    }
}
```

Keep `root` as a `PathBuf` so every existing `SkillPackage::load(&package.root)` call remains unchanged. In `rejects_required_file_symlink_that_escapes_package_root`, replace the manually constructed external path with `package.sibling("outside-permissions.json")` and remove its manual cleanup; it remains outside `package.root` while `TempDir` owns cleanup.

- [ ] **Step 4: Replace the template unit-test directory wrapper**

In the `#[cfg(test)]` module in `crates/skilltape-core/src/template.rs`, remove `AtomicU64`, `Ordering`, and the manual `Drop`. Use the existing test-only dependency instead:

```rust
struct TestDir(TempDir);

impl TestDir {
    fn new(label: &str) -> Self {
        let prefix = format!("skilltape-template-{label}-");
        Self(
            tempfile::Builder::new()
                .prefix(&prefix)
                .tempdir()
                .expect("test directory"),
        )
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.path().join(path)
    }
}
```

Keep the three existing template tests and their assertions exactly as they are. Do not touch `create_skill_template` or `write_new_file`.

- [ ] **Step 5: Replace the deterministic compiler package root helper**

In `crates/skilltape-compiler/tests/deterministic_compile.rs`, remove `AtomicU64`, `Ordering`, `PathBuf`, `NEXT_TEMP_ID`, and `temporary_package_root`. Add `use tempfile::TempDir;`. In `generated_package_support_files_load_and_lint_cleanly`, create the owned parent and package root as follows:

```rust
let temporary = TempDir::new().expect("temporary package parent");
let root = temporary.path().join("package");
fs::create_dir(&root).expect("package root");
```

Delete `fs::remove_dir_all(&root)` at the end of that test because `temporary` now owns cleanup. Preserve all generated-package lint assertions.

- [ ] **Step 6: Run the focused regression suites and inspect the diff**

Run:

```bash
cargo test -p skilltape-core --test package_validation
cargo test --locked -p skilltape-core --test package_validation
cargo fmt --all -- --check
cargo test --locked -p skilltape-core template::tests
cargo test --locked -p skilltape-compiler --test deterministic_compile
git diff -- Cargo.lock
git diff --check
```

Expected: every command passes. The first command updates the lockfile's package metadata if necessary; the following locked commands prove that the lockfile is synchronized. Confirm that `Cargo.lock` changes only the `skilltape-core` and `skilltape-compiler` package dependency lists and that no dependency version changes are introduced.

- [ ] **Step 7: Commit the isolated fixture change**

```bash
git add crates/skilltape-core/Cargo.toml \
  crates/skilltape-core/tests/package_validation.rs \
  crates/skilltape-core/src/template.rs \
  crates/skilltape-compiler/Cargo.toml \
  crates/skilltape-compiler/tests/deterministic_compile.rs \
  Cargo.lock
git commit -m "test: use managed temporary fixture directories"
```

### Task 2: Add Console collection boundary evidence

**Files:**

- Modify: `apps/skilltape-console-api/tests/routes.rs`

**Interfaces:**

- Consumes: `fixture()`, `request()`, the existing `ConsoleReadModel::new`, and Unix `symlink` support.
- Produces: regression coverage for `directory_names` and `count_files`, the exact sinks behind alerts 8 and 9.
- Does not change: `apps/skilltape-console-api/src/read_model.rs` or the Console HTTP contract.

- [ ] **Step 1: Add a focused collection test before changing any production code**

Add this Unix-only test after `symlinked_storage_resource_is_forbidden`:

```rust
#[cfg(unix)]
#[tokio::test]
async fn collection_endpoints_ignore_symlinked_storage_entries() {
    use std::os::unix::fs::symlink;

    let (_temp, root) = fixture();
    let outside_directory = root.join("outside-directory");
    let outside_receipt = root.join("outside-receipt.json");
    fs::create_dir(&outside_directory).expect("outside directory");
    fs::write(&outside_receipt, b"{}").expect("outside receipt");
    symlink(
        &outside_directory,
        root.join(".skilltape/tapes/linked"),
    )
    .expect("tape symlink");
    symlink(
        &outside_receipt,
        root.join(".skilltape/receipts/linked.json"),
    )
    .expect("receipt symlink");

    let (status, _body, workspaces) = request(&root, "/api/v1/workspaces", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(workspaces["items"][0]["tape_count"], 1);
    assert_eq!(workspaces["items"][0]["receipt_count"], 1);

    let (status, _body, tapes) =
        request(&root, "/api/v1/workspaces/default/tapes", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tapes["items"].as_array().expect("tape items").len(), 1);
    assert_eq!(tapes["items"][0]["id"], "tape-a");
}
```

The expected result is a pass on the existing implementation. This is intentional: it establishes executable proof for a CodeQL false-positive classification and must not be accompanied by a production code change.

- [ ] **Step 2: Expand route-ID coverage to the cross-platform forms already rejected by `validate_id`**

Replace the single request in `unsafe_ids_and_invalid_pagination_return_structured_errors` with this loop while retaining its existing pagination assertions:

```rust
for uri in [
    "/api/v1/tapes/%2E%2E%2Foutside/events",
    "/api/v1/tapes/%5Coutside/events",
    "/api/v1/tapes/C%3Aoutside/events",
] {
    let (status, _body, error) = request(&root, uri, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
    assert_eq!(error["schema"], "skilltape.dev/api-error/v1");
    assert_eq!(error["error"]["code"], "unsafe_id");
}
```

Do not add a path normalizer or alter route behavior. If any form does not return `BAD_REQUEST`, stop this task and treat it as a confirmed Console boundary defect.

- [ ] **Step 3: Run the focused Console tests**

Run:

```bash
cargo test --locked -p skilltape-console-api --test routes collection_endpoints_ignore_symlinked_storage_entries -- --exact
cargo test --locked -p skilltape-console-api --test routes unsafe_ids_and_invalid_pagination_return_structured_errors -- --exact
cargo test --locked -p skilltape-console-api --test routes
git diff --check
```

Expected: all commands pass, and no production source file changes are present.

- [ ] **Step 4: Commit the evidence-only regression coverage**

```bash
git add apps/skilltape-console-api/tests/routes.rs
git commit -m "test: cover Console symlink collection boundaries"
```

### Task 3: Publish the individual CodeQL audit ledger

**Files:**

- Create: `docs/security/codeql-path-audit.md`
- Modify: `docs/README.md`

**Interfaces:**

- Consumes: the 65-alert inventory from CodeQL analysis SHA `7366106f4af752c5a9df9f18a8e10eec245b9898`, Task 1 fixture changes, Task 2 Console proof, and existing source controls.
- Produces: a public English ledger with one row per alert, a reproducible scan-query command, an exact disposition rationale, and no secret or machine-specific data.

- [ ] **Step 1: Create the audit document with a stable record format**

Start `docs/security/codeql-path-audit.md` with this metadata and scope statement:

```markdown
# CodeQL path-safety audit

**Date:** 2026-08-08

**Status:** Pending final CodeQL scan and individual GitHub dispositions

This record accounts for the 65 `rust/path-injection` and `py/path-injection`
alerts opened by CodeQL default setup for main commit
`7366106f4af752c5a9df9f18a8e10eec245b9898`. It does not change CodeQL query
configuration, threat model, or alert severity.
```

Then add a table with exactly these columns:

```markdown
| Alert | Rule | Source and sink | Trust boundary | Verified controls | Decision | Final state |
| ---: | --- | --- | --- | --- | --- | --- |
```

Create one table row for every integer from `1` through `65`. Use only repository-relative code paths. For every row, copy the matching decision from the alert decision map above; use `Pending matching scan` in the final-state column until Task 5.

- [ ] **Step 2: Record concrete evidence for each decision category**

Use these exact evidence anchors in the individual rows rather than vague phrases:

```markdown
- Release packaging: `safe_component`, `require_directory`, `require_file`, and
  `copy_ui` in `scripts/package_release.py`; symlink fixture coverage in
  `scripts/test_release_package.py`.
- Console: `ConsoleReadModel::new`, `storage_child`, `ensure_safe_path`,
  `validate_id`, `directory_names`, and `count_files` in
  `apps/skilltape-console-api/src/read_model.rs`; Task 2 route tests.
- Capture/Tape: `resolve_workspace`, `resolve_output`, and
  `validate_default_output` in `crates/skilltape-cli/src/capture_command.rs`;
  `TapeStore::create` and `TapeStore::open` in `crates/skilltape-tape/src/store.rs`;
  Capture output and symlink tests.
- Replay: `validate_relative_path`, `resolve_under`, and
  `ensure_no_symlink_ancestors` in `crates/skilltape-runner/src/workspace.rs`;
  package validation and replay symbolic-link tests.
- Test fixtures: the `TempDir` ownership change in Task 1 and the test target
  path for alert 49.
```

For alert 49, state that its sink is a test-only fake adapter using the static relative names `inputs/fixture.txt` and `scripts/emit.sh`. For alerts 17-46 and 63, state that Task 1 removes the environment-derived source; if a matching scan still reports them, the only allowed disposition is `used in tests` with this documented reason.

- [ ] **Step 3: Add a reproducible read-only CodeQL query and the non-goals**

Add the following command and requirements below the ledger:

```bash
gh api 'repos/Chumaniac/skilltape/code-scanning/alerts?state=open&per_page=100' \
  --jq '.[] | [.number, .rule.id, .most_recent_instance.location.path, (.most_recent_instance.location.start_line | tostring)] | @tsv'
```

State explicitly that the audit must not use global dismissals, `@codeql` comments, CodeQL configuration exclusions, or a threat-model downgrade. State that a CodeQL alert without sufficient source-to-sink evidence remains open.

- [ ] **Step 4: Add the contributor-facing documentation link**

Under the `Contributors` section in `docs/README.md`, add a bullet with the label `CodeQL path-safety audit`, the relative target `security/codeql-path-audit.md`, and the description `evidence and disposition criteria for path-injection findings.`

- [ ] **Step 5: Run the documentation quality gates**

Run:

```bash
git add docs/security/codeql-path-audit.md docs/README.md
git grep -n --perl-regexp '[\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}]' -- . ':!Cargo.lock' || true
python3 - <<'PY'
from pathlib import Path
import re
import subprocess
import sys

link_pattern = re.compile(r'(?<!!)\[[^\]]+\]\(([^)\s]+)(?:\s+["\'][^"\']*["\'])?\)')
files = subprocess.check_output(["git", "ls-files", "-z", "--", "*.md"]).split(b"\0")
missing = []
for raw_path in files:
    if not raw_path:
        continue
    source = Path(raw_path.decode())
    text = source.read_text(encoding="utf-8")
    for match in link_pattern.finditer(text):
        target = match.group(1).strip("<>")
        if not target or target.startswith("#") or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", target):
            continue
        target_path = target.split("#", 1)[0].split("?", 1)[0]
        if target_path and not (source.parent / target_path).exists():
            missing.append(f"{source}: {target}")
if missing:
    print("Missing Markdown targets:", file=sys.stderr)
    print("\n".join(missing), file=sys.stderr)
    raise SystemExit(1)
PY
git diff --check
```

Expected: no natural-language Chinese match, no missing Markdown target, and no whitespace error.

- [ ] **Step 6: Commit the public audit record**

```bash
git commit -m "docs: record CodeQL path safety audit"
```

### Task 4: Run the branch verification gate and publish a reviewable change

**Files:**

- Modify: no new repository file unless a verification failure exposes a demonstrated defect covered by the approved design.

**Interfaces:**

- Consumes: the three commits from Tasks 1-3, the existing GitHub remote, and CodeQL default setup.
- Produces: a clean, fully verified branch and a draft pull request for protected-branch review. It does not merge `main`.

- [ ] **Step 1: Run the complete local validation gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
python3 -m unittest scripts.test_release_package scripts.test_release_workflow
git diff --check
git status --short
```

Expected: formatting, Clippy, every Rust test, both Python release tests, and the whitespace check pass; `git status --short` is empty after the task commits.

- [ ] **Step 2: Reconcile the local alert inventory before external publication**

Run the read-only query from Task 3 and save its output in the task report, not a tracked file. Confirm that it lists exactly alerts `1` through `65`, all on the baseline `main` analysis, before any dismissal mutation occurs.

- [ ] **Step 3: Publish the branch and create a draft pull request**

Use the GitHub publication workflow to push `codex/skilltape-codeql-path-audit` and create a draft pull request targeting `main` with this title and body summary:

```text
Title: docs: audit CodeQL path safety

Summary:
- replaces hand-built temporary test fixtures with owned TempDir fixtures;
- adds Console collection and route-ID boundary regression coverage;
- records an individual evidence-backed disposition for all 65 CodeQL path-injection alerts;
- does not change product path semantics, CodeQL configuration, or main-branch protection.
```

Do not use administrator bypass, merge the pull request, or dismiss any GitHub alert in this step.

- [ ] **Step 4: Wait for and inspect the branch CodeQL result**

After the branch is pushed, locate the completed CodeQL run for the branch with:

```bash
gh run list --branch codex/skilltape-codeql-path-audit --limit 20 \
  --json databaseId,name,status,conclusion,url,headSha
```

Use the completed run whose `headSha` matches `git rev-parse HEAD`. Query the branch alert set and compare it with the ledger:

```bash
gh api 'repos/Chumaniac/skilltape/code-scanning/alerts?state=open&per_page=100&ref=refs/heads/codex/skilltape-codeql-path-audit' \
  --jq '.[] | [.number, .rule.id, .most_recent_instance.location.path, (.most_recent_instance.location.start_line | tostring)] | @tsv'
```

Record whether the Task 1 fixture-source alerts are absent or still present. If a previously classified `false positive` resolves unexpectedly, retain the ledger classification but do not manufacture a code change. If an alert shows a new untrusted path source, stop and return to the approved design before changing code.

- [ ] **Step 5: Commit no generated scan output**

Keep run URLs, alert counts, and command output in the execution report or pull-request description. Do not add generated JSON, raw SARIF, local paths, or environment data to the repository.

### Task 5: Complete main-branch alert disposition after protected-branch integration

**Dependency:** Task 4 must be reviewed and merged through the repository's configured protected-branch workflow. This task must not bypass the required review.

**Files:**

- Modify: `docs/security/codeql-path-audit.md` only in a follow-up documentation commit, if final scan URLs and alert state need to be recorded.

**Interfaces:**

- Consumes: the merged commit SHA, a completed `main` CodeQL run for that SHA, and the per-alert ledger decision.
- Produces: individually fixed or dismissed alerts with an evidence-based public rationale, plus a final ledger state.

- [ ] **Step 1: Verify the merged SHA and matching main CodeQL scan**

Run:

```bash
git fetch origin main
git rev-parse origin/main
gh run list --branch main --limit 20 --json databaseId,name,status,conclusion,url,headSha
```

Select the completed CodeQL run whose `headSha` equals `origin/main`. Do not act on an earlier scan.

- [ ] **Step 2: Identify only the residual open path-injection alerts**

Run:

```bash
gh api 'repos/Chumaniac/skilltape/code-scanning/alerts?state=open&per_page=100' \
  --jq '.[] | select(.rule.id == "rust/path-injection" or .rule.id == "py/path-injection") | [.number, .rule.id, .most_recent_instance.location.path, (.most_recent_instance.location.start_line | tostring)] | @tsv'
```

For each result, match the alert number and source/sink to its ledger row. Any mismatch, added alert, or missing evidence remains open and is reported as a blocker.

- [ ] **Step 3: Apply only individual, category-correct dispositions**

For each residual alert, set `alert_number` to the checked alert number and use one request with the matching exact reason/comment form. Do not send range-based or bulk requests.

```bash
alert_number=8
gh api --method PATCH "repos/Chumaniac/skilltape/code-scanning/alerts/$alert_number" \
  -f state=dismissed \
  -f dismissed_reason='false positive' \
  -f dismissed_comment='Path audit: the value is confined by the documented canonical-root, relative-path, or symlink control; see docs/security/codeql-path-audit.md.'
```

```bash
alert_number=49
gh api --method PATCH "repos/Chumaniac/skilltape/code-scanning/alerts/$alert_number" \
  -f state=dismissed \
  -f dismissed_reason='used in tests' \
  -f dismissed_comment='Path audit: this source-to-sink flow is test-only and uses a controlled fixture; see docs/security/codeql-path-audit.md.'
```

```bash
alert_number=1
gh api --method PATCH "repos/Chumaniac/skilltape/code-scanning/alerts/$alert_number" \
  -f state=dismissed \
  -f dismissed_reason="won't fix" \
  -f dismissed_comment='Path audit: this is an explicit trusted local operator or release-tool path, not product data crossing an untrusted boundary; see docs/security/codeql-path-audit.md.'
```

Use `false positive` only for the Console, Replay, Capture metadata, and Console binary-check categories; `used in tests` only for a residual test-target alert; and `won't fix` only for the explicit local Capture/Tape or trusted release-tool categories. Never dismiss an alert classified as a confirmed or unresolved boundary defect.

- [ ] **Step 4: Record the final state and verify closure**

Update each ledger row's final-state column with `Fixed by scan`, `Dismissed: false positive`, `Dismissed: used in tests`, or `Dismissed: won't fix`, and add the matching CodeQL run URL and merged SHA to the document's status section. Submit that documentation-only update through the same protected-branch workflow.

After that follow-up is merged, run the residual-open query from Step 2 again. Expected: no open `rust/path-injection` or `py/path-injection` alert remains without an explicit, evidence-backed blocker in the ledger.

- [ ] **Step 5: Final verification report**

Report the merged SHA, CodeQL run URL, number of alerts fixed by the scan, number dismissed by each reason, remaining blockers if any, and the fresh local/CI verification evidence. Do not claim completion while an unexplained open path-injection finding remains.
