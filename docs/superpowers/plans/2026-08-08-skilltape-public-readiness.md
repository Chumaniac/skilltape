# SkillTape public-readiness implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the public SkillTape repository announcement-ready by hardening
future releases, improving collaboration surfaces, and applying the dependent
GitHub settings.

**Architecture:** Keep product code unchanged. The release workflow becomes the
single source of truth for immutable tags, pinned workflow dependencies, SPDX
SBOMs, and GitHub attestations. Repository files define collaboration behavior;
remote GitHub settings enforce the same safety policy after the repository PR
has merged.

**Tech Stack:** GitHub Actions, GitHub REST API, GitHub artifact attestations,
Syft through `anchore/sbom-action`, Python `unittest`, Markdown, YAML, Rust
1.97.1, Node.js 22, and Playwright.

## Global constraints

- Preserve the current public `v0.1.0` release as checksum-verified history;
  do not claim it is attested or attach retroactive provenance.
- Pin every `uses:` reference to a full SHA with an adjacent version comment.
- Future releases must require an existing matching `v<version>` tag and must
  use `gh release create --verify-tag`.
- Generate SPDX JSON SBOMs for each release archive and attest each archive
  with its corresponding SBOM.
- Do not upload Tapes, Receipts, logs, environment values, or credentials.
- Preserve the explicit Windows Replay/Verify fail-closed limitation.
- Keep all new prose in English and do not include personal contact details.

---

### Task 1: Harden the release workflow and prove its contract

**Files:**

- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/skill-verify.yml`
- Modify: `scripts/test_release_workflow.py`

**Interfaces:**

- Consumes: the `v*` tag trigger, `workflow_dispatch.inputs.version`, release
  archives emitted by `scripts/package_release.py`, and GitHub's artifact
  attestation permissions.
- Produces: SHA-pinned Actions, a tag validation job, archive-level SPDX SBOMs,
  archive/SBOM attestations, and a release command that refuses absent tags.

- [ ] **Step 1: Write a failing workflow-contract test**

Update `scripts/test_release_workflow.py` to require full-SHA references for
all actions and the following release fragments:

```text
prepare:
git ls-remote --exit-code origin "refs/tags/$tag"
manual release must be dispatched from the matching v<version> tag
gh release create "$tag" --repo "$GITHUB_REPOSITORY" --verify-tag
anchore/sbom-action@
actions/attest@
attestations: write
id-token: write
sbom-path:
```

Run `python3 scripts/test_release_workflow.py` and confirm it fails only
because the current workflow still uses mutable action tags and lacks the new
release controls.

- [ ] **Step 2: Pin every workflow action**

Replace each `uses:` reference with these immutable references and retain the
version comment on the same line:

```text
actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7
actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7
actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7
actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8
dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772 # master
anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0
actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4
```

- [ ] **Step 3: Add immutable-release preparation**

Add a read-only `prepare` job before `build`. It resolves the version from a
tag push or manual `version` input, rejects unsafe values, verifies that
`refs/tags/v<version>` exists, and verifies that its peeled commit equals
`GITHUB_SHA`. Export `version` and `tag` as job outputs. Make `build` depend
on `prepare`; use its `version` output for archive names. Make `publish` depend
on `prepare` and `build`, and make `windows-installer` depend on `prepare` and
`publish`.

- [ ] **Step 4: Generate and attest release SBOMs**

Make the package step expose its archive path and adjacent
`<archive>.spdx.json` path as outputs. Generate an SPDX JSON SBOM from the
archive with `anchore/sbom-action`, keeping its own artifact/release upload
features disabled. Add `id-token: write` and `attestations: write` to the
build-job permissions. Attest the archive with `actions/attest` using both
`subject-path` and `sbom-path`. Preserve `release/*` as the upload path so
the release receives archive and SBOM assets.

- [ ] **Step 5: Verify and commit**

Run:

```bash
python3 scripts/test_release_workflow.py
ruby -e 'require "yaml"; Dir[".github/workflows/*.yml"].each { |f| YAML.load_file(f); puts "YAML OK #{f}" }'
git diff --check
```

Commit with `ci: harden immutable release provenance`.

### Task 2: Add public collaboration and dependency maintenance files

**Files:**

- Create: `CODE_OF_CONDUCT.md`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`
- Create: `.github/ISSUE_TEMPLATE/feature_request.yml`
- Create: `.github/pull_request_template.md`
- Create: `.github/dependabot.yml`
- Modify: `CONTRIBUTING.md`
- Modify: `SECURITY.md`

**Interfaces:**

- Consumes: GitHub Issue Forms, Dependabot version-update configuration, and
  the existing private security disclosure policy.
- Produces: guided public contribution intake that never routes exploits into
  public issues, routine updates for Cargo/npm/Actions, and an enforceable
  conduct policy.

- [ ] **Step 1: Create the forms and policy files**

Use GitHub Issue Forms for bugs and feature requests. Disable blank issues.
The bug form must require version, platform, reproduction, expected behavior,
and actual behavior, and must warn against including sensitive Tape/Receipt
content. The configuration must link security reports to `SECURITY.md` rather
than an Issue Form.

Write a concise Contributor Covenant-derived Code of Conduct that directs
reports to the repository owner through their GitHub profile without exposing
personal contact information. Add a PR template covering scope, tests,
documentation, and secret-free evidence.

- [ ] **Step 2: Add routine dependency updates**

Create `.github/dependabot.yml` with weekly schedules for Cargo,
`apps/skilltape-console` npm dependencies, and GitHub Actions. Limit each
ecosystem to five open version-update pull requests and use the `chore`
commit-message prefix.

- [ ] **Step 3: Align contributor and security instructions**

Add links to the Code of Conduct, Issue Forms, PR template expectations, and
Dependabot automation in `CONTRIBUTING.md`. In `SECURITY.md`, state that
GitHub Private Vulnerability Reporting is enabled and remains the first choice
for security reports after the remote setting is applied.

- [ ] **Step 4: Validate and commit**

Run a YAML parser over the new forms and Dependabot file, then run:

```bash
rg -n -i 'TODO|TBD|<owner>|<repo>|<your-' CODE_OF_CONDUCT.md CONTRIBUTING.md SECURITY.md .github
git diff --check
```

Commit with `docs: add public contribution governance`.

### Task 3: Improve public onboarding and release documentation

**Files:**

- Modify: `README.md`
- Modify: `docs/guides/installation.md`
- Modify: `docs/release-readiness.md`

**Interfaces:**

- Consumes: the immutable-release workflow, real GitHub repository URL, and
  documented platform limits.
- Produces: runnable clone/install instructions and future-release integrity
  guidance without retroactively changing `v0.1.0` claims.

- [ ] **Step 1: Replace public placeholders and stale branch links**

Use `https://github.com/Chumaniac/skilltape.git` in the README clone command
and use an explicit `https://github.com/Chumaniac/skilltape/tree/main/docs`
documentation-index link. Keep relative source-tree links elsewhere unless
they would target a non-existent branch.

- [ ] **Step 2: Document release integrity accurately**

Explain that future release workflows require matching protected tags, include
an archive-local SPDX SBOM, generate GitHub attestations, and publish
checksums. Add `gh attestation verify` examples for a downloaded future archive
and its SPDX predicate. State explicitly that `v0.1.0` has checksums but was
published before provenance/SBOM generation was added.

- [ ] **Step 3: Validate and commit**

Run the documentation style guide's Markdown-link and English-prose checks,
then run `git diff --check`. Commit with `docs: clarify public release verification`.

### Task 4: Merge the reviewed branch and apply GitHub settings

**Files:**

- No repository files; apply only after Tasks 1–3 are merged.

**Interfaces:**

- Consumes: green CI/CodeQL checks from the merged branch and the action SHA
  references committed by Task 1.
- Produces: a protected single-maintainer flow, private disclosure channel,
  SHA-pinned Actions enforcement, CodeQL merge gating, repository topics, and
  cleaned-up post-merge branches.

- [ ] **Step 1: Apply security and collaboration settings**

Enable Private Vulnerability Reporting. Require all eight checks on `main`:
`Rust (ubuntu-latest)`, `Rust (macos-14)`, `Console (ubuntu-latest)`,
`Console (macos-latest)`, `Analyze (actions)`, `Analyze (javascript-typescript)`,
`Analyze (python)`, and `Analyze (rust)`. Keep PRs and conversation resolution
required, set zero external approvals for the sole maintainer, and enforce the
same protections for administrators.

- [ ] **Step 2: Lock action execution policy**

Enable GitHub's full-SHA pinning requirement only after the merged workflows
contain the immutable references from Task 1. Keep the default `GITHUB_TOKEN`
read-only and prohibit Actions from approving pull requests.

- [ ] **Step 3: Set public metadata**

Set repository topics to `rust`, `cli`, `developer-tools`, `workflow-automation`,
`security`, `reproducible-builds`, `local-first`, and `github-actions`. Enable
automatic deletion of merged PR branches. Keep Discussions disabled and leave
Homepage blank until a non-repository project website exists.

- [ ] **Step 4: Verify settings and release behavior**

Read each changed setting through `gh api`, verify `main` protection includes
all eight checks, ensure CodeQL/Dependabot/secret scanning remain enabled, and
run a manual-release dry-run only against an existing protected tag when a
future version is intentionally released.

### Task 5: Create and upload a public social-preview image

**Files:**

- Create: `docs/assets/skilltape-social-preview.png`

**Interfaces:**

- Consumes: the local-first/replay-verifiable product message and the public
  repository name.
- Produces: a 1280×640 GitHub social-preview asset with no third-party logos,
  user data, or unsupported platform claims.

- [ ] **Step 1: Generate the approved visual asset**

Create a dark, technically clean 1280×640 image with the words `SkillTape`,
`Capture → Compile → Replay → Verify`, and `Local-first, replay-verifiable
Agent Skills`. Use abstract Tape/workflow lines and no vendor or platform
logos.

- [ ] **Step 2: Upload and verify**

Upload the image in the repository's Social preview settings and retain the
tracked source asset. Confirm GitHub displays the repository preview without
altering project visibility or other repository metadata.
