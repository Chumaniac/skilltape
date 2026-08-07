# SkillTape English Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Translate the entire tracked SkillTape repository to English natural-language content, organize its documentation, repair stale links and release statements, and verify that the migration does not change product behavior.

**Architecture:** Keep the root files as public entry points, use `docs/README.md` as the navigation index, and separate guides, stable references, design records, approved specs/plans, and historical execution reports. Translate prose in place, move only the historical SDD reports into the approved archive path, and update every inbound link in the same change.

**Tech Stack:** Markdown, Rust, TypeScript/React, YAML, JSON, GitHub Actions, Git, Rust toolchain, Node.js/Vite/Playwright, and the existing Python/Bash release fixture scripts.

## Global Constraints

- English is the canonical source language; every tracked Markdown file and every public natural-language description must be English.
- Preserve schema identifiers, enum values, protocol field names, CLI flags, commands, paths, URLs, package names, target triples, environment variable names, hashes, timestamps, and historical commit identifiers exactly.
- Do not change Rust execution, policy, sandbox, schema validation, export behavior, dependency versions, or release publication behavior.
- Preserve historical dates, task IDs, evidence, acceptance criteria, and limitations; add archival context when a historical status is no longer current.
- Keep user-provided fixture payloads unchanged when they test arbitrary bytes rather than document product behavior.
- Use relative Markdown links for repository files and update links whenever a file moves or a stale path is removed.
- Treat `main` at commit `bdd82937fc652190917a8259098bc92ae48553cb` as the current merged implementation state; no versioned GitHub Release has been published.

---

### Task 1: Create the documentation index and archive the historical reports

**Files:**
- Create: `docs/README.md`
- Create: `docs/guides/README.md`
- Create: `docs/reference/README.md`
- Create: `docs/superpowers/reports/README.md`
- Archive: `docs/superpowers/reports/2026-08-04-skilltape-foundation/task-4-diagnostic-report.md`
- Archive: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-2-report.md`
- Archive: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-4-report.md`
- Archive: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-5-report.md`
- Archive: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-6-report.md`
- Archive: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-7-report.md`

**Interfaces:**
- Consumes: the existing Markdown tree, the approved migration specification, and the six tracked historical reports.
- Produces: stable navigation pages and the `docs/superpowers/reports/` archival path used by the root README and future contributors.

- [ ] **Step 1: Define the user-facing documentation map**

  Write `docs/README.md` with audience paths for users, Skill authors, adapter authors, contributors, and release maintainers. Link only to files that exist in the repository, including the root entry points, guides, references, design records, specs, plans, and reports.

- [ ] **Step 2: Add section-level indexes**

  Write `docs/guides/README.md`, `docs/reference/README.md`, and `docs/superpowers/reports/README.md`. Each index must state its scope in English and link to the exact files in its directory. The reference index must link to `schemas/` and identify the versioned schema families without duplicating their definitions.

- [ ] **Step 3: Move the historical reports without editing their evidence**

  Move the six report files to the matching `docs/superpowers/reports/<date-project>/` directories. Preserve their headings, dates, task identifiers, command output, and commit references; only update relative links if a report contains one.

- [ ] **Step 4: Update inbound links and archive language**

  Resolve every historical report reference to its `docs/superpowers/reports/` path. Mark the reports as historical execution evidence in the archive index and leave the root `.superpowers/` directory absent from the tracked documentation map.

- [ ] **Step 5: Verify the structure**

  Run:

  ```bash
  test -f docs/README.md
  test -f docs/guides/README.md
  test -f docs/reference/README.md
  test -f docs/superpowers/reports/README.md
  stale_root='superpowers'
  stale_leaf='sdd'
  ! git grep -n "$stale_root/$stale_leaf"
  git diff --check
  ```

- [ ] **Step 6: Commit the navigation and archive boundary**

  ```bash
  git add docs/README.md docs/guides/README.md docs/reference/README.md docs/superpowers/reports .superpowers
  git commit -m "docs: organize documentation indexes and reports"
  ```

### Task 2: Translate and reconcile public entry points and release documents

**Files:**
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `SECURITY.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/guides/installation.md`
- Modify: `docs/release-readiness.md`

**Interfaces:**
- Consumes: current CLI commands, platform/security behavior, release workflow, merged-main CI run `31149247700`, and the documentation index from Task 1.
- Produces: English public documentation whose release and support claims match the merged `main` branch.

- [ ] **Step 1: Translate the root README without changing command semantics**

  Translate headings, workflow descriptions, quick-start prose, CLI table labels, CI notes, security notes, design goals, and links. Keep every command, flag, schema identifier, target, environment variable, and code block exact. Add a direct link to `docs/README.md` and remove outdated Draft PR or unmerged-branch wording.

- [ ] **Step 2: Normalize contributor and security terminology**

  Translate `CONTRIBUTING.md` and `SECURITY.md`. Use one vocabulary for Capture, Compile, Lint, Replay, Verify, Receipt, Tape, sandbox, permission, redaction, fixture, and release. Preserve the security boundary that Windows Replay/Verify fails closed and that Linux/macOS are the supported restricted-executor CI matrix.

- [ ] **Step 3: Reconcile changelog and release readiness**

  Translate `CHANGELOG.md` and update the release candidate entry to state that the implementation is merged on `main`, CI is green, and the versioned GitHub Release remains unpublished. Rewrite `docs/release-readiness.md` around the current merged-main evidence, current CI run `31149247700`, local verification, the remaining four-target release workflow, Windows PowerShell fixture, archive/checksum review, and explicit release-tag approval.

- [ ] **Step 4: Update the installation guide as the release source of truth**

  Translate `docs/guides/installation.md`. Remove the stale claim that the worktree has no Git remote. Keep the exact release archive layout, installer safety behavior, platform prerequisites, Console discovery variables, and local verification commands. Link the guide to the new indexes and the release workflow.

- [ ] **Step 5: Check public-document consistency**

  Run:

  ```bash
  rg -n --pcre2 '[\p{Han}]' README.md CONTRIBUTING.md SECURITY.md CHANGELOG.md docs/guides docs/release-readiness.md
  rg -n 'Draft PR|codex/skilltape-foundation|worktree has no|no Git remote' README.md CHANGELOG.md docs/guides docs/release-readiness.md || true
  git diff --check
  ```

- [ ] **Step 6: Commit the public documentation set**

  ```bash
  git add README.md CONTRIBUTING.md SECURITY.md CHANGELOG.md docs/guides/installation.md docs/release-readiness.md
  git commit -m "docs: translate public guides and release status"
  ```

### Task 3: Normalize reference, schema, example, and fixture documentation

**Files:**
- Modify: `docs/reference/tape-format.md`
- Modify: `docs/reference/adapter-api.md`
- Modify: `examples/minimal-skill/README.md`
- Modify: `examples/minimal-skill/SKILL.md`
- Modify: `examples/minimal-skill/skilltape.yaml`
- Modify: `tests/fixtures/invalid-skill/README.md`
- Modify: `tests/fixtures/invalid-skill/SKILL.md`
- Modify: `tests/fixtures/invalid-skill/skilltape.yaml`

**Interfaces:**
- Consumes: JSON schemas under `schemas/`, the current Rust model validation, and the reference index from Task 1.
- Produces: English normative reference pages and consistent package descriptions that remain valid under the existing lint fixtures.

- [ ] **Step 1: Translate the Tape reference and mark normative rules**

  Translate `docs/reference/tape-format.md`. Keep the JSON example byte-for-byte equivalent, preserve all event kinds and schema identifiers, and label path, sequence, redaction, locking, and recovery rules as normative behavior. Link the related schema families through `docs/reference/README.md` and `schemas/`.

- [ ] **Step 2: Review the Exporter and Plugin API for English consistency**

  Normalize `docs/reference/adapter-api.md` headings, terminology, exit-code descriptions, capability names, and security checklist. Keep the Rust trait, JSON fields, paths, and protocol values exact.

- [ ] **Step 3: Normalize example and invalid-fixture descriptions**

  Ensure both minimal and invalid fixture documentation use English descriptions and accurately distinguish the valid example from the intentionally rejected package. Preserve invalid values that are required to exercise policy or schema errors.

- [ ] **Step 4: Validate fixtures after description changes**

  Run:

  ```bash
  cargo run --locked -p skilltape-cli -- lint examples/minimal-skill --strict
  if cargo run --locked -p skilltape-cli -- lint tests/fixtures/invalid-skill; then
    exit 1
  fi
  python3 scripts/test_release_package.py
  git diff --check
  ```

- [ ] **Step 5: Commit reference and fixture documentation**

  ```bash
  git add docs/reference examples/minimal-skill tests/fixtures/invalid-skill
  git commit -m "docs: normalize references and fixture descriptions"
  ```

### Task 4: Translate the product design record

**Files:**
- Modify: `docs/design/2026-08-04-skilltape-design.md`

**Interfaces:**
- Consumes: the implemented crate boundaries, CLI behavior, Console capabilities, schema names, and security rules already documented by the current code.
- Produces: an English historical product and architecture record that remains useful for maintainers without changing its dated research or design decisions.

- [ ] **Step 1: Translate the design headings and status metadata**

  Translate the title, status, date, project note, section headings, tables, diagrams, and captions. Preserve the original date and version label. Add a short English archival note stating that this is a historical design record and that current implementation status is tracked by `docs/release-readiness.md`.

- [ ] **Step 2: Translate product, architecture, security, and rollout prose**

  Translate all natural-language paragraphs and lists in the 1,045-line design record. Preserve product names, command names, schema identifiers, code blocks, file paths, and URLs exactly. Use the approved vocabulary for Tape, Skill, Run, Receipt, Workflow IR, permission policy, redaction, Replay, Verify, and Console.

- [ ] **Step 3: Repair design-record links and current-state claims**

  Replace links to missing documents with valid links to `docs/README.md`, `docs/guides/`, `docs/reference/`, `docs/superpowers/specs/`, or `docs/superpowers/plans/`. Keep historical intent explicit and avoid presenting design-only functionality as a published release feature.

- [ ] **Step 4: Verify the design record**

  Run:

  ```bash
  ! rg -n --pcre2 '[\p{Han}]' docs/design/2026-08-04-skilltape-design.md
  git diff --check -- docs/design/2026-08-04-skilltape-design.md
  ```

- [ ] **Step 5: Commit the translated design record**

  ```bash
  git add docs/design/2026-08-04-skilltape-design.md
  git commit -m "docs: translate the product design record"
  ```

### Task 5: Translate and repair specs, plans, and historical execution context

**Files:**
- Modify: `docs/superpowers/specs/2026-08-05-skilltape-full-product-design.md`
- Modify: `docs/superpowers/plans/2026-08-04-skilltape-foundation.md`
- Modify: `docs/superpowers/plans/2026-08-05-skilltape-full-product.md`
- Modify: `docs/superpowers/plans/2026-08-07-skilltape-console-release.md`
- Modify: `docs/superpowers/specs/2026-08-07-skilltape-console-release-design.md`
- Modify: `docs/superpowers/specs/2026-08-07-skilltape-documentation-design.md`
- Modify: `docs/superpowers/reports/README.md`
- Modify: `docs/superpowers/reports/2026-08-04-skilltape-foundation/task-4-diagnostic-report.md`
- Modify: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-2-report.md`
- Modify: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-4-report.md`
- Modify: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-5-report.md`
- Modify: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-6-report.md`
- Modify: `docs/superpowers/reports/2026-08-05-skilltape-full-product/task-7-report.md`

**Interfaces:**
- Consumes: the approved English migration spec, the existing implementation commits, and the archived report files moved in Task 1.
- Produces: an English-only historical engineering record with valid links and explicit separation between proposed design, implementation plan, and completed evidence.

- [ ] **Step 1: Translate the full-product design and plan**

  Translate every Chinese heading, status line, paragraph, table, checklist label, and explanatory comment in `2026-08-05-skilltape-full-product-design.md` and `2026-08-05-skilltape-full-product.md`. Preserve task numbering, interfaces, schema values, commands, test names, and acceptance criteria exactly.

- [ ] **Step 2: Audit the foundation plan's historical links**

  Translate any remaining non-English prose in `2026-08-04-skilltape-foundation.md`. Replace its references to nonexistent `2026-08-04-skilltape-capture.md`, `skilltape-compiler.md`, `skilltape-verify.md`, and `skilltape-console.md` plans with valid current documentation links or plain historical references that do not resolve to missing files. Update its final plan pointer to an existing plan or the documentation index.

- [ ] **Step 3: Normalize the Console release design and plan**

  Review the already-English Console design and plan for stale branch, release, or installation claims. Link them to the current installation guide, release-readiness page, and release workflow; do not alter the release workflow's trigger or publish behavior.

- [ ] **Step 4: Preserve and label report evidence**

  Keep report command lines, output counts, commit identifiers, toolchain paths, and failure/root-cause evidence exact. Translate only prose and labels. Add an archive note to `docs/superpowers/reports/README.md` explaining that reports are historical evidence and are not current implementation requirements.

- [ ] **Step 5: Verify the entire superpowers tree**

  Run:

  ```bash
  ! rg -n --pcre2 '[\p{Han}]' docs/superpowers
  stale_root='superpowers'
  stale_leaf='sdd'
  ! git grep -n "$stale_root/$stale_leaf"
  git diff --check -- docs/superpowers
  ```

- [ ] **Step 6: Commit the translated historical records**

  ```bash
  git add docs/superpowers
  git commit -m "docs: translate and archive engineering records"
  ```

### Task 6: Complete repository-wide description and language cleanup

**Files:**
- Inspect and modify only if the language scan identifies a natural-language mismatch: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/skill-verify.yml`, `apps/skilltape-console/index.html`, `apps/skilltape-console/src/**/*.tsx`, `apps/skilltape-console/tests/**/*.ts`, `crates/**/*.rs`, `schemas/**/*.json`, `examples/**/*.yaml`, and `tests/fixtures/**/*.yaml`.
- Modify: `docs/documentation-style.md`
- Modify: `CONTRIBUTING.md` to link the documentation style guide.

**Interfaces:**
- Consumes: all translated documents and the approved language policy.
- Produces: one discoverable English documentation standard and no remaining Chinese natural-language content or inconsistent public descriptions.

- [ ] **Step 1: Write the contributor-facing documentation style guide**

  Create `docs/documentation-style.md` covering English-only prose, sentence-case headings, product vocabulary, normative versus informative wording, command/code-block preservation, relative-link rules, status/date requirements, security-copy review, and the required language/link checks.

- [ ] **Step 2: Link the style guide from contributor documentation**

  Add an English documentation section to `CONTRIBUTING.md` that links to `docs/documentation-style.md` and requires documentation changes to run the repository language and link checks.

- [ ] **Step 3: Scan every tracked file for residual non-English content**

  Run:

  ```bash
  git grep -n --perl-regexp '[\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}]' -- . ':!Cargo.lock' || true
  ```

  For each match, translate natural-language text to English while leaving arbitrary fixture bytes and protocol values unchanged. If a match is a false positive from an unrelated Unicode symbol, document that it is not Han text and leave it untouched.

- [ ] **Step 4: Verify metadata and UI copy**

  Confirm that the GitHub repository description, Console HTML metadata, workflow input descriptions, package descriptions, installer messages, and Console test assertions are English and consistent with the public README. Do not make a GitHub metadata write because the current repository description is already English; only update local files if a mismatch exists.

- [ ] **Step 5: Commit the language standard and cleanup**

  ```bash
  git add docs/documentation-style.md CONTRIBUTING.md .github apps crates schemas examples tests
  git commit -m "docs: standardize repository language and copy"
  ```

### Task 7: Run the complete documentation migration verification

**Files:**
- Inspect: all tracked files and all Markdown links.
- Modify only if verification identifies a direct migration regression.

**Interfaces:**
- Consumes: the completed documentation migration and all prior task commits.
- Produces: fresh evidence for language purity, link integrity, formatting, builds, tests, and release fixtures.

- [ ] **Step 1: Check tracked-file language purity and filenames**

  ```bash
  git ls-files -z | xargs -0 -n1 basename | LC_ALL=C grep -n '[^ -~]' && exit 1 || true
  if git grep -n --perl-regexp '[\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}]' -- . ':!Cargo.lock'; then
    exit 1
  fi
  ```

  The first command is informational for filename review; the second is the authoritative content gate. No non-ASCII filename is currently expected, but any discovered filename must be renamed with all inbound links updated.

- [ ] **Step 2: Validate Markdown links**

  Run a repository-local link audit that extracts relative Markdown targets, ignores anchors and external URLs, resolves each target relative to its source file, and exits non-zero for a missing target. The audit must include moved report paths and the historical plan links repaired in Task 5.

- [ ] **Step 3: Run formatting and Rust verification**

  ```bash
  PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo fmt --all -- --check
  PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo clippy --locked --workspace --all-targets -- -D warnings
  PATH=/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH CI=1 cargo test --locked --workspace -- --test-threads=1
  ```

- [ ] **Step 4: Run Console and release fixture verification**

  ```bash
  npm ci --prefix apps/skilltape-console
  npm --prefix apps/skilltape-console run build
  npm --prefix apps/skilltape-console test
  python3 scripts/test_release_package.py
  python3 scripts/test_release_workflow.py
  bash -n scripts/install.sh scripts/test_install.sh scripts/smoke_console.sh
  bash scripts/test_install.sh
  ```

  `bash -n` is applied only to shell scripts; PowerShell syntax is verified by the existing release workflow fixture and must not be passed to Bash.

- [ ] **Step 5: Review the final diff and status claims**

  ```bash
  git diff --check origin/main...HEAD
  git status --short --branch
  git log --oneline --decorate -12
  ```

  Confirm that no source behavior, schema value, dependency, release trigger, or user-owned feature-worktree change was modified. Confirm that public release documents describe the current merged main state and that no release tag or GitHub Release was created.

- [ ] **Step 6: Create the final migration commit and prepare publication**

  If all prior task commits are clean and the verification output is fresh, create one final documentation verification commit only when needed for a direct fix. Otherwise retain the focused task commits, summarize the evidence, and ask before pushing the documentation commits to `origin/main`.

## Dependency and risk notes

- Task 1 must finish before Tasks 2–5 because all indexes and report links depend on the final archive paths.
- Task 2 must finish before Task 7 because it establishes the current release-status vocabulary and facts.
- Task 3 depends on the reference index and must run the fixture lint gate after changing package descriptions.
- Task 4 and Task 5 can be translated independently after Task 1, but both must use the vocabulary fixed by Task 2.
- Task 6 depends on the public and historical documents so its repository-wide scan can distinguish true residual content from already-reviewed fixture data.
- Task 7 is the final gate; no completion or push claim is valid before its fresh results are available.
- The largest risks are semantic drift in security/reference prose, broken historical links, and accidentally changing code-block or fixture values. Review those sections against the current implementation and use the existing automated gates before committing.
