# SkillTape Post-Release Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkboxes for tracking.

**Goal:** Close the verified v0.1.0 release follow-up items without changing SkillTape product behavior.

**Architecture:** Reconcile only historical documentation statements, upgrade GitHub-hosted workflow actions to their Node.js 24-compatible major versions, and keep release verification unchanged. Treat the separate foundation worktree as user-owned state; inspect it for reporting but do not overwrite or delete its edits.

**Tech Stack:** Markdown, GitHub Actions YAML, Rust 1.97.1, Homebrew, Bash, Python, Ruby, Node.js/Vite/Playwright, and the existing Rust workspace.

## Global Constraints

- Keep the current `main` release baseline and all product behavior unchanged.
- Preserve historical dates, commit identifiers, evidence, and acceptance criteria; add explicit archival context when a historical status is no longer current.
- Keep `actions/checkout`, `actions/setup-node`, `actions/setup-python`, `actions/upload-artifact`, and `actions/download-artifact` on Node.js 24-compatible major versions supported by GitHub-hosted runners.
- Do not modify or delete the two uncommitted reports in `.worktrees/skilltape-foundation`.
- Do not expose credentials or change repository visibility, billing, or security settings without an explicit external-settings decision.

---

### Task 1: Reconcile historical release statements

**Files:**
- Modify: `docs/superpowers/specs/2026-08-07-skilltape-console-release-design.md`
- Modify: `docs/superpowers/plans/2026-08-07-skilltape-english-documentation.md`

**Interfaces:**
- Consumes: the current published `v0.1.0` release and tag evidence.
- Produces: historical records that remain factually faithful to their original time while pointing readers to the current release state.

- [x] **Step 1: Mark the release design statement as historical**

  Replace the current sentence that says no versioned GitHub Release has been
  published with an archival note that says this was true when the design was
  written and links to the current `v0.1.0` release.

- [x] **Step 2: Update the documentation migration baseline**

  Keep commit `bdd82937fc652190917a8259098bc92ae48553cb` as the historical
  migration baseline, and add the current `main` commit `4c484fdb` plus the
  published `v0.1.0` release URL as a later-state note.

- [x] **Step 3: Verify documentation consistency**

  Run:

  ```bash
  rg -n -i "no versioned GitHub Release|has not been published|not yet.*release|no Git remote" README.md CHANGELOG.md CONTRIBUTING.md SECURITY.md docs
  git diff --check
  ```

  Expected: only intentionally historical wording remains, with adjacent
  archival context; no public current-status document reports a missing release.

### Task 2: Upgrade workflow actions to Node.js 24-compatible majors

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/skill-verify.yml`

**Interfaces:**
- Consumes: the existing workflow job matrix, permissions, inputs, artifact names, and release commands.
- Produces: identical CI/release behavior using `checkout@v7`, `setup-node@v7`, `setup-python@v7`, `upload-artifact@v7`, and `download-artifact@v8`.

- [x] **Step 1: Replace action major tags without changing job logic**

  Update only the `uses:` version tags listed above. Preserve runner labels,
  matrices, permissions, cache inputs, artifact names, shell commands, and
  release API calls.

- [x] **Step 2: Run static workflow checks**

  Run:

  ```bash
  ruby -e 'require "yaml"; Dir[".github/workflows/*.yml"].each { |f| YAML.load_file(f); puts "YAML OK #{f}" }'
  python3 -m unittest scripts.test_release_workflow
  rg -n 'actions/(checkout|setup-node|setup-python|upload-artifact|download-artifact)@' .github/workflows
  ```

  Expected: YAML and workflow unit tests pass, and every listed action uses the
  planned Node.js 24-compatible major.

### Task 3: Repair and verify the local Rust toolchain

**Files:**
- No repository files; operate only on the existing Homebrew installation.

**Interfaces:**
- Consumes: the installed Homebrew Rust 1.97.1 toolchain and LLVM formula.
- Produces: a working default `cargo`/`rustc` invocation, or a documented non-destructive fallback if Homebrew cannot repair the installation.

- [x] **Step 1: Reinstall the missing LLVM runtime**

  Run `brew reinstall llvm` and do not remove unrelated formulas or alter the
  repository. Confirm that the missing `libLLVM.dylib` is restored.

- [x] **Step 2: Verify the default compiler**

  Run `rustc -Vv`, `cargo -V`, `cargo fmt --all -- --check`, and the locked
  workspace Clippy command. If the Homebrew compiler remains broken, retain the
  project-pinned rustup toolchain as the verified fallback and record the exact
  failure instead of changing project code.

### Task 4: Execute the full post-change verification gate

**Files:**
- No additional files; verify the changes from Tasks 1–3.

- [x] **Step 1: Run Rust and Console checks**

  Run the locked Rust format/Clippy/workspace test gate and the Console
  dependency install, production build, and Playwright tests.

- [x] **Step 2: Run release and installer checks**

  Run `scripts/test_release_package.py`, `scripts.test_release_workflow`, the
  installer fixture, shell syntax checks, and the packaged Console smoke test.

- [x] **Step 3: Verify repository state**

  Confirm `git diff --check`, Markdown local links, English-only tracked
  content, the absence of tracked placeholder markers, a clean `main` worktree, and
  equality with `origin/main` before any publication step.

- [x] **Step 4: Report external governance items separately**

  Do not change branch protection, Dependabot, code scanning, or secret
  scanning in this implementation pass. Report their current availability and
  the GitHub plan limitation as a separate follow-up.
