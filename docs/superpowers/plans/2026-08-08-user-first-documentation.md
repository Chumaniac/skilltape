# User-first Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make SkillTape's public documentation lead a new user from a clear
Beta-value statement to a real, locally verified result within five minutes.

**Architecture:** Keep `README.md` as the outcome-first landing page and move
the expanded first-run path into a new Quickstart guide. A small documentation
contract test guards the public promises, while a reusable shell journey test
executes the same Capture → Compile → Lint → Verify commands against a built
CLI. Historical plans and reports stay in the repository but leave the user
navigation path.

**Tech Stack:** Markdown, SVG, Bash, Python 3.12 standard library, Rust CLI,
GitHub Actions.

## Global Constraints

- Public prose is English; internal implementation records stay unlinked from
  user documentation.
- The README uses the exact primary promise: `Turn a real local workflow into
  a reviewable Agent Skill you can replay and verify before you share it.`
- Label the project **Beta** before the first command.
- Do not publish an npm launcher, imply that `npx` starts SkillTape, or claim
  that an online hosted executor exists.
- Use the fixed `v0.1.0` native release and the fixed installer-source commit
  `beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6` in copyable installation examples.
- A visual demo must derive from a real deterministic CLI journey, redact
  workspace-specific values as `<workspace>`, include accessible SVG metadata,
  and link to a text transcript.
- Linux/macOS may document Replay/Verify only with their restricted-executor
  prerequisite; Windows documents Capture/Compile/Lint/Export and says
  Replay/Verify fail closed.
- Do not claim automatic `.env` loading. Show only environment variables that
  the installer or Console actually reads.
- Preserve historical specifications, release-readiness records, audits, and
  reports; remove them only from user navigation.

---

## File structure

| Path | Responsibility |
| --- | --- |
| `README.md` | Product landing page: value, Beta boundary, visual demo, first commands, and focused links. |
| `docs/guides/quickstart.md` | Complete first-result path for Unix and Windows, expected output, and platform-safe fallback. |
| `docs/guides/installation.md` | Native install, update, prerequisites, troubleshooting, and factual environment-variable configuration. |
| `docs/README.md` | Task-oriented documentation entry point. |
| `docs/guides/README.md` | Small guide index using user actions rather than a file catalog. |
| `docs/assets/quickstart-terminal.txt` | Normalized transcript from the real deterministic journey. |
| `docs/assets/quickstart-terminal.svg` | Accessible terminal screenshot rendered from the normalized transcript. |
| `scripts/test_user_documentation.py` | Fast static contract for user-entry copy, links, assets, and removed headings. |
| `scripts/test_quickstart.sh` | Executable Unix equivalent of the documented Capture → Compile → Lint → Verify flow. |
| `.github/workflows/ci.yml` | Runs both documentation gates after building the CLI. |

## Task 1: Add a user-documentation contract and an executable quickstart gate

**Files:**
- Create: `scripts/test_user_documentation.py`
- Create: `scripts/test_quickstart.sh`
- Test: `scripts/test_user_documentation.py`
- Test: `scripts/test_quickstart.sh`

**Consumes:** Existing `skilltape` CLI commands and the full-journey behavior
proved by `tests/integration/full_journey.rs`.

**Produces:** A static documentation contract and a reusable CLI journey gate
that later tasks can invoke locally and in CI.

- [ ] **Step 1: Write the failing documentation contract**

Create `scripts/test_user_documentation.py` with Python's standard-library
`unittest`. Define these constants and tests before changing any documentation:

```python
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
README = ROOT / "README.md"
DOCS_INDEX = ROOT / "docs" / "README.md"
GUIDES_INDEX = ROOT / "docs" / "guides" / "README.md"
QUICKSTART = ROOT / "docs" / "guides" / "quickstart.md"
TRANSCRIPT = ROOT / "docs" / "assets" / "quickstart-terminal.txt"
VISUAL = ROOT / "docs" / "assets" / "quickstart-terminal.svg"

PRIMARY_PROMISE = (
    "Turn a real local workflow into a reviewable Agent Skill you can replay "
    "and verify before you share it."
)
RETIRED_README_HEADINGS = (
    "## CI and Skill repository integration",
    "## Security, compatibility, and benchmarks",
    "## Design goals",
)
```

Implement the initial test module with this behavior:

```python
import re
import unittest
from pathlib import Path

LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")

class UserDocumentationContractTests(unittest.TestCase):
    def test_readme_has_a_value_statement_beta_and_user_entry_links(self):
        text = README.read_text(encoding="utf-8")
        self.assertIn(PRIMARY_PROMISE, text)
        self.assertIn("**Beta**", text)
        self.assertIn("docs/assets/quickstart-terminal.svg", text)
        self.assertIn("docs/guides/quickstart.md", text)
        self.assertIn(
            "https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0", text
        )

    def test_visual_demo_is_accessible_and_has_a_transcript(self):
        self.assertTrue(TRANSCRIPT.is_file())
        self.assertTrue(VISUAL.is_file())
        transcript = TRANSCRIPT.read_text(encoding="utf-8")
        visual = VISUAL.read_text(encoding="utf-8")
        for fragment in ("<title>", "<desc>", "Capture", "Verify", "<workspace>"):
            self.assertIn(fragment, transcript + visual)
        self.assertNotIn("/tmp/", transcript + visual)

    def test_readme_does_not_promote_retired_implementation_sections(self):
        text = README.read_text(encoding="utf-8")
        for heading in RETIRED_README_HEADINGS:
            self.assertNotIn(heading, text)

    def test_quickstart_has_a_unix_first_result_and_safe_boundary(self):
        self.assertTrue(QUICKSTART.is_file())
        text = QUICKSTART.read_text(encoding="utf-8")
        for fragment in (
            "## macOS and Linux",
            "bwrap",
            "sandbox-exec",
            "beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6",
            "installation.md",
        ):
            self.assertIn(fragment, text)

    def test_public_relative_markdown_links_resolve(self):
        for source in (README, QUICKSTART):
            self.assertTrue(source.is_file())
            for destination in LINK_RE.findall(source.read_text(encoding="utf-8")):
                target = destination.split("#", 1)[0].strip().strip("<>")
                if not target or "://" in target or target.startswith("mailto:"):
                    continue
                self.assertTrue(
                    (source.parent / target).resolve().exists(),
                    f"{source.relative_to(ROOT)} links to missing {target}",
                )

if __name__ == "__main__":
    unittest.main()
```

The first test must require `PRIMARY_PROMISE`, `**Beta**`, the SVG Markdown
reference, `docs/guides/quickstart.md`, and the `v0.1.0` GitHub Release URL.
The visual test must require `<title>`, `<desc>`, `Capture`, `Verify`,
`<workspace>`, and no literal `/tmp/` in either asset. The Quickstart test
must require the macOS/Linux heading, `bwrap`, `sandbox-exec`, the fixed
release installer source commit, and a link to `installation.md`.
The initial relative-link test must inspect `README.md` and Quickstart once it
exists, skip anchors and external URLs, and resolve Markdown destinations
relative to their source file. It must fail with the source file and unresolved
destination. Task 3 expands its surface list after the remaining user guides
and indexes exist.

- [ ] **Step 2: Run the contract before implementation**

Run:

```bash
python3 scripts/test_user_documentation.py
```

Expected: FAIL because `docs/guides/quickstart.md`, the demo assets, and the
new README structure do not exist yet.

- [ ] **Step 3: Write the executable first-result journey**

Create `scripts/test_quickstart.sh` as a Bash script with `set -Eeuo pipefail`.
It accepts one required CLI path, rejects a missing or non-executable argument,
creates a `mktemp -d` root with an `EXIT` cleanup trap, and runs these exact
operations:

```bash
skilltape_bin="$1"
demo_root="$(mktemp -d "${TMPDIR:-/tmp}/skilltape-quickstart.XXXXXX")"
workspace="$demo_root/workspace"
tape="$demo_root/tape"
skill="$demo_root/skill"
receipt="$demo_root/receipt.json"
mkdir -p "$workspace"

"$skilltape_bin" capture demo --workspace "$workspace" --command /bin/echo \
  --output "$tape" --yes --json > "$demo_root/capture.json"
"$skilltape_bin" compile "$tape" --output "$skill" > "$demo_root/compile.txt"
"$skilltape_bin" lint "$skill" --strict --json > "$demo_root/lint.json"
"$skilltape_bin" verify "$skill" --receipt "$receipt" --json > "$demo_root/receipt-output.json"
```

Use `python3 - "$demo_root" <<'PY'` to assert all of the following:

```python
assert capture["ok"] is True
assert capture["name"] == "demo"
assert capture["event_count"] >= 4
assert lint["errors"] == []
assert receipt_output["schema"] == "skilltape.dev/receipt/v1"
assert receipt_output["status"] == "succeeded"
assert receipt_output == json.loads((root / "receipt.json").read_text())
assert (root / "skill" / "SKILL.md").is_file()
assert (root / "skill" / "workflow.yaml").is_file()
```

Print exactly `quickstart journey passed` after those checks.

- [ ] **Step 4: Run the journey against the built CLI**

Run:

```bash
cargo build --locked -p skilltape-cli
bash scripts/test_quickstart.sh target/debug/skilltape
```

Expected: PASS with `quickstart journey passed` on a Linux/macOS environment
where the restricted executor is available.

- [ ] **Step 5: Commit the gates**

```bash
git add scripts/test_user_documentation.py scripts/test_quickstart.sh
git commit -m "test: add user documentation journey gates"
```

## Task 2: Build the outcome-first README and real terminal demo

**Files:**
- Create: `docs/assets/quickstart-terminal.txt`
- Create: `docs/assets/quickstart-terminal.svg`
- Create: `docs/guides/quickstart.md`
- Modify: `README.md`
- Test: `scripts/test_user_documentation.py`
- Test: `scripts/test_quickstart.sh`

**Consumes:** Task 1 documentation constants and the exact CLI journey.

**Produces:** The public landing page and a visual proof of the first result.

- [ ] **Step 1: Record the real normalized transcript**

Run the Task 1 journey once with `target/debug/skilltape`, then create
`docs/assets/quickstart-terminal.txt`. State in its first line that only the
temporary workspace path and nonrepeatable IDs/hashes are normalized. Include
these exact command forms and result facts:

```text
$ skilltape capture demo --workspace <workspace> --command /bin/echo --output <workspace>/tape --yes --json
{"ok":true,"name":"demo","tape_path":"<workspace>/tape","event_count":4,"command":"/bin/echo","exit_code":0}
$ skilltape compile <workspace>/tape --output <workspace>/skill
Compiled skill at <workspace>/skill
$ skilltape lint <workspace>/skill --strict --json
{"files_checked":6,"errors":[],"warnings":[]}
$ skilltape verify <workspace>/skill --receipt <workspace>/receipt.json --json
{"schema":"skilltape.dev/receipt/v1","status":"succeeded","steps":[{"step_id":"exec-0001","status":"succeeded","exit_code":0}]}
```

- [ ] **Step 2: Create the accessible terminal screenshot**

Create `docs/assets/quickstart-terminal.svg` with a 1280×720 dark terminal
layout. Include a `<title>` of `SkillTape first verified Skill`, a `<desc>`
that says the image shows a local command captured, compiled, linted, and
verified with a successful Receipt, and text rendered from the transcript's
four command/result pairs. Use SVG-safe escaped text; do not include any local
absolute path, username, token, or dynamic run ID.

- [ ] **Step 3: Replace README content and add the Unix Quickstart**

Rewrite `README.md` in this order:

```markdown
# SkillTape

> **Beta** — Turn a real local workflow into a reviewable Agent Skill you can replay and verify before you share it.

SkillTape captures a command you already run, turns it into a reviewable Skill,
and produces a Receipt that tells you whether the isolated replay succeeded.
It runs locally; the first result needs no account, API key, model provider, or
configuration file.

![Terminal output from SkillTape Capture, Compile, Lint, and Verify](docs/assets/quickstart-terminal.svg)

[Watch the 30-second terminal demo](docs/assets/quickstart-terminal.txt) · [Download v0.1.0](https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0)
```

Follow it with `## Get a verified Skill in five minutes`, the fixed Unix
installer source commit `beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6`, the public
release base URL, and the Task 1 four-command journey. Use an `installer_path`
variable and `curl --fail --location --silent --show-error --output` rather
than `curl | bash`; set `SKILLTAPE_VERSION="0.1.0"`, run `bash
"$installer_path"`, then prepend `$HOME/.local/bin` to `PATH`.

Add a short `## What works today` section that states the Linux/macOS sandbox
prerequisites and the exact Windows boundary. Add `## Use SkillTape when` and
`## It is not yet for` with plain product decisions, then link to Quickstart,
installation, the minimal example, security, and contributing. Task 3 adds the
configuration link after that guide exists.
Do not reintroduce the retired README headings, CI explanation, benchmark,
design record, implementation plan, or release-readiness link.

Create `docs/guides/quickstart.md` with these initial sections:

```markdown
# Get a verified Skill in five minutes
## What you will get
## macOS and Linux
## If Verify cannot start
## What the result means
## Next steps
```

Repeat the fixed-release install block and the Task 1 four-command journey,
then link to `installation.md` for platform and update details. State that
Debian/Ubuntu users can install `bwrap` with `sudo apt-get install bubblewrap`,
that macOS uses `/usr/bin/sandbox-exec`, and that Capture, Compile, Lint, and
Export remain usable if Verify cannot start.

- [ ] **Step 4: Verify the public landing page contract and first journey**

Run:

```bash
python3 scripts/test_user_documentation.py
bash scripts/test_quickstart.sh target/debug/skilltape
```

Expected: both PASS; the static test confirms the exact promise, Beta label,
asset, release link, Unix Quickstart, and retired-heading removal.

- [ ] **Step 5: Commit the landing page**

```bash
git add README.md docs/guides/quickstart.md docs/assets/quickstart-terminal.txt \
  docs/assets/quickstart-terminal.svg
git commit -m "docs: lead SkillTape with a first verified result"
```

## Task 3: Create a task-oriented Quickstart, install guide, and indexes

**Files:**
- Create: `docs/guides/configuration.md`
- Modify: `docs/guides/installation.md`
- Modify: `docs/README.md`
- Modify: `docs/guides/README.md`
- Modify: `docs/guides/quickstart.md`
- Test: `scripts/test_user_documentation.py`

**Consumes:** Task 2's user-facing terms, fixed release URL, visual asset, and
platform boundaries.

**Produces:** A discoverable documentation path that separates first use,
installation, optional configuration, reference, and contributor material.

- [ ] **Step 1: Write the failing navigation assertions**

Add `test_user_indexes_route_to_tasks_not_internal_plans` so it requires
all of these relative links in the user indexes:

```text
guides/quickstart.md
guides/installation.md
guides/configuration.md
../examples/minimal-skill/README.md
../SECURITY.md
../CONTRIBUTING.md
```

Require `docs/guides/quickstart.md` to link back to `installation.md` and
`configuration.md`. Extend `test_quickstart_has_a_unix_first_result_and_safe_boundary`
to require `whoami.exe` and the exact Windows phrase `Replay/Verify fail
closed`. Extend `test_public_relative_markdown_links_resolve` to inspect
`README.md`, both indexes, Quickstart, Installation, and Configuration. Keep
the forbiddance for internal plans and reports.

Add this exact index test and replace the initial link-test source tuple with
the six public documents:

```python
PUBLIC_DOCUMENTS = (
    README,
    DOCS_INDEX,
    GUIDES_INDEX,
    QUICKSTART,
    ROOT / "docs" / "guides" / "installation.md",
    ROOT / "docs" / "guides" / "configuration.md",
)

def test_user_indexes_route_to_tasks_not_internal_plans(self):
    required = (
        "guides/quickstart.md",
        "guides/installation.md",
        "guides/configuration.md",
        "../examples/minimal-skill/README.md",
        "../SECURITY.md",
        "../CONTRIBUTING.md",
    )
    combined = DOCS_INDEX.read_text(encoding="utf-8") + GUIDES_INDEX.read_text(encoding="utf-8")
    for fragment in required:
        self.assertIn(fragment, combined)
    for retired in ("superpowers/", "release-readiness.md", "CodeQL path-safety audit"):
        self.assertNotIn(retired, combined)
```

Replace `test_public_relative_markdown_links_resolve` with:

```python
def test_public_relative_markdown_links_resolve(self):
    for source in PUBLIC_DOCUMENTS:
        self.assertTrue(source.is_file())
        for destination in LINK_RE.findall(source.read_text(encoding="utf-8")):
            target = destination.split("#", 1)[0].strip().strip("<>")
            if not target or "://" in target or target.startswith("mailto:"):
                continue
            self.assertTrue(
                (source.parent / target).resolve().exists(),
                f"{source.relative_to(ROOT)} links to missing {target}",
            )
```

- [ ] **Step 2: Run the expanded contract before rewriting indexes**

Run:

```bash
python3 scripts/test_user_documentation.py
```

Expected: FAIL because Configuration, the Windows-safe Quickstart path, and
task-oriented indexes do not exist yet.

- [ ] **Step 3: Write Quickstart and Configuration**

Extend `docs/guides/quickstart.md` with this additional section after the
Unix fallback section:

```markdown
## Windows: create and export a Skill
```

The Windows section uses PowerShell, downloads the fixed-commit
`scripts/install.ps1`, sets:

```powershell
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/Chumaniac/skilltape/releases/download"
$env:SKILLTAPE_VERSION = "0.1.0"
$env:SKILLTAPE_TARGET = "x86_64-pc-windows-msvc"
```

Then runs `capture` with `--command whoami.exe`, `compile`, `lint --strict
--json`, and `export --target generic --json`. State exactly:
`Replay/Verify fail closed on Windows, so this path deliberately stops after Export.`
Add `installation.md` and `configuration.md` links in Quickstart's `Next
steps` section.

Create `docs/guides/configuration.md` with only real environment variables.
State that no `.env` file is loaded automatically, then provide separate Unix
`export` and PowerShell templates for `SKILLTAPE_VERSION`,
`SKILLTAPE_RELEASE_BASE_URL`, `SKILLTAPE_INSTALL_DIR`, `SKILLTAPE_TARGET`,
`SKILLTAPE_CONSOLE_API_BIN`, and `SKILLTAPE_CONSOLE_UI_DIST`. Identify the
Console variables as optional packaged-asset overrides, not required setup.

- [ ] **Step 4: Rewrite installation and navigation around user tasks**

Rename `docs/guides/installation.md` to the heading `# Install and update
SkillTape`. Its first paragraph links to Quickstart and says the native release
is the default path. Keep release archive naming, checksum behavior, supported
targets, platform prerequisites, update/custom-directory examples, and Console
override facts. Move source builds beneath a late `## Build from source
(contributors)` heading. Remove its `Local CI`, `Capture → Compile → Verify`,
`Local verification`, and `GitHub Actions` user-facing sections.

Rewrite `docs/README.md` as a compact action list headed `# Use SkillTape`:

```markdown
- [Get a verified first Skill](guides/quickstart.md)
- [Install or update SkillTape](guides/installation.md)
- [Configure optional paths](guides/configuration.md)
- [Start from a minimal Skill](../examples/minimal-skill/README.md)
- [Read the stable format reference](reference/README.md)
- [Read the security policy](../SECURITY.md)
- [Contribute](../CONTRIBUTING.md)
```

Rewrite `docs/guides/README.md` as `# Start using SkillTape` with the three
guide links above and no release-maintainer or file-inventory prose.

- [ ] **Step 5: Verify navigation and content boundaries**

Run:

```bash
python3 scripts/test_user_documentation.py
git diff --check
```

Expected: PASS with no internal-plan/release-readiness routing in user indexes.

- [ ] **Step 6: Commit the guide reorganization**

```bash
git add docs/README.md docs/guides/README.md docs/guides/quickstart.md \
  docs/guides/configuration.md docs/guides/installation.md scripts/test_user_documentation.py
git commit -m "docs: organize guidance around first use"
```

## Task 4: Enforce the public documentation in CI and verify the release path

**Files:**
- Modify: `.github/workflows/ci.yml`
- Test: `scripts/test_user_documentation.py`
- Test: `scripts/test_quickstart.sh`
- Test: `scripts/test_install.sh`

**Consumes:** Task 1 reusable gates and Task 2–3 completed documentation.

**Produces:** CI evidence that public documentation remains aligned with a real
local result, plus a fresh release-installer proof for this change.

- [ ] **Step 1: Add and run the failing CI contract assertion**

Extend `scripts/test_user_documentation.py` with
`test_ci_runs_user_documentation_and_quickstart_gates`. Read
`.github/workflows/ci.yml` and require these exact command fragments in the
`Validate release and installer fixtures` step:

```text
python3 scripts/test_user_documentation.py
cargo build --locked -p skilltape-cli
bash scripts/test_quickstart.sh target/debug/skilltape
```

Implement the assertion as:

```python
def test_ci_runs_user_documentation_and_quickstart_gates(self):
    workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    for fragment in (
        "python3 scripts/test_user_documentation.py",
        "cargo build --locked -p skilltape-cli",
        "bash scripts/test_quickstart.sh target/debug/skilltape",
    ):
        self.assertIn(fragment, workflow)
```

Run:

```bash
python3 scripts/test_user_documentation.py
```

Expected: FAIL because the CI workflow has not yet invoked the two new gates.

- [ ] **Step 2: Add the CI invocation and prove the contract passes**

In `.github/workflows/ci.yml`, amend the `Validate release and installer
fixtures` step after `python3 scripts/test_release_workflow.py` to contain:

```bash
python3 scripts/test_user_documentation.py
cargo build --locked -p skilltape-cli
bash scripts/test_quickstart.sh target/debug/skilltape
```

Run:

```bash
python3 scripts/test_user_documentation.py
```

Expected: PASS because CI now executes the public documentation and executable
quickstart gates.

- [ ] **Step 3: Run the complete local verification set**

Run:

```bash
python3 scripts/test_user_documentation.py
cargo build --locked -p skilltape-cli
bash scripts/test_quickstart.sh target/debug/skilltape
bash scripts/test_install.sh
python3 scripts/test_release_package.py
python3 scripts/test_release_workflow.py
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
npm --prefix apps/skilltape-console test
```

Expected: every gate passes. Existing Console colour-environment warnings may
appear but are not failures.

- [ ] **Step 4: Prove the actual published Unix release install**

Use a fresh temporary install root without writing into the user home:

```bash
fresh_root="$(mktemp -d /tmp/skilltape-public-install.XXXXXX)"
installer_path="$fresh_root/skilltape-install-v0.1.0.sh"
curl --fail --location --silent --show-error --output "$installer_path" \
  "https://raw.githubusercontent.com/Chumaniac/skilltape/beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6/scripts/install.sh"
SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download" \
SKILLTAPE_VERSION="0.1.0" \
SKILLTAPE_INSTALL_DIR="$fresh_root/bin" \
bash "$installer_path"
bash scripts/test_quickstart.sh "$fresh_root/bin/skilltape"
```

Expected: the installer reports the selected verified archive and the released
binary produces a successful Receipt through the exact quickstart journey.

- [ ] **Step 5: Run Markdown-link and working-tree checks**

Run the public Markdown relative-link audit through the contract, then:

```bash
python3 scripts/test_user_documentation.py
git diff --check
git status --short
```

Expected: all relative links resolve, no whitespace errors remain, and only
the intended files are modified.

- [ ] **Step 6: Commit CI enforcement**

```bash
git add .github/workflows/ci.yml scripts/test_user_documentation.py
git commit -m "ci: verify the public quickstart journey"
```

## Task 5: Review, publish, and verify the user-facing documentation change

**Files:**
- Review: `README.md`
- Review: `docs/guides/quickstart.md`
- Review: `docs/guides/installation.md`
- Review: `docs/guides/configuration.md`
- Review: `docs/assets/quickstart-terminal.svg`
- Review: `.github/workflows/ci.yml`

**Consumes:** All completed implementation and validation tasks.

**Produces:** A reviewed pull request whose checks prove the public docs match
real behavior.

- [ ] **Step 1: Perform a user-path review**

Review the first three README sections without opening any internal document.
Confirm a visitor sees, in order: the exact promise, Beta label, visual demo,
download link, first command, expected tangible result, and platform boundary.
Confirm all wording refers to user outcomes rather than implementation status
or an inventory of repository files.

- [ ] **Step 2: Run focused review commands**

```bash
python3 scripts/test_user_documentation.py
bash scripts/test_quickstart.sh target/debug/skilltape
git diff --check
git diff origin/main...HEAD -- README.md docs .github/workflows/ci.yml scripts
```

Expected: the diff is limited to user documentation, assets, documentation
tests, and their CI invocation.

- [ ] **Step 3: Commit any review correction**

If review reveals a concrete defect, add a regression assertion to
`scripts/test_user_documentation.py` or `scripts/test_quickstart.sh`, make the
minimal correction, rerun the focused commands, and commit with a `docs:` or
`test:` message that names the corrected user-facing behavior.

- [ ] **Step 4: Push and open a draft pull request**

```bash
git push -u origin codex/skilltape-user-first-docs
gh pr create --draft --base main --head codex/skilltape-user-first-docs \
  --title "docs: make SkillTape usable in five minutes"
```

The PR body must name the public promise, Beta boundary, visual demo, native
release installer, Quickstart journey, documentation contract, and the exact
validation commands. It must say that no hosted demo or npm launcher was
introduced.

- [ ] **Step 5: Wait for and verify GitHub checks**

Run:

```bash
gh pr checks --watch
```

Expected: the four CI matrix checks and four CodeQL checks pass. Only then mark
the PR ready and merge under the repository's documented solo-maintainer
workflow.

## Plan self-review

- **Spec coverage:** Tasks 1–2 implement the value statement, Beta label,
  native release choice, real visual demo, and five-minute result. Task 3
  implements configuration, platform boundaries, and user-only navigation.
  Task 4 proves command and release behavior. Task 5 enforces a focused
  review/publish path.
- **No-placeholder scan:** The plan contains no unresolved markers. Exact
  files, commands, expected results, environment variable names, and public
  copy are specified.
- **Consistency:** `scripts/test_user_documentation.py` is the static contract
  in every task; `scripts/test_quickstart.sh` is the executable user journey;
  CI runs both after the CLI is built. The README and Quickstart use the same
  fixed release version and installer-source commit.
