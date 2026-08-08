# YAML workflow parser gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fragile handwritten workflow YAML scanner with a locked,
test-only YAML parser gate that reliably enforces immutable action references
and the separate release provenance/SBOM attestations.

**Architecture:** The Rust CI job installs an exactly pinned, hash-verified
PyYAML wheel on a fixed Python 3.12 runtime. `scripts/test_release_workflow.py`
uses `yaml.safe_load` and recursive value traversal for `uses` mappings and
structured release-step assertions; raw text remains only for the tag shell
script and publication-time revalidation placement.

**Tech Stack:** Python 3.12, PyYAML 6.0.3, GitHub Actions, Dependabot, Ruby
YAML syntax check, unittest.

## Global Constraints

- Install only `PyYAML==6.0.3` binary wheels with `--require-hashes`.
- Use Python 3.12 on the GitHub-hosted Ubuntu x86_64 and macOS arm64 Rust jobs.
- Keep all GitHub Action references pinned to forty-character SHAs with the
  existing adjacent version comments.
- Reject every parsed `uses` mapping whose string value is not the existing
  strict `owner/repository@` followed by forty lowercase hexadecimal characters
  form.
- Do not change release behavior, tags, published `v0.1.0` assets, Windows
  Replay/Verify behavior, or remote repository settings in this plan.
- Preserve English-only tracked prose and run the repository documentation
  gates using the binary-safe `git grep -I` form.

## File structure

| Path | Responsibility |
| --- | --- |
| `scripts/requirements-workflow-tests.txt` | Hash-locked parser dependency for CI and local workflow-contract tests. |
| `.github/workflows/ci.yml` | Fixed Python setup and dependency install before workflow-contract tests. |
| `.github/dependabot.yml` | Weekly pip updates for the new test-only requirement. |
| `scripts/test_release_workflow.py` | Parsed workflow traversal, action pinning, parsed attestation contracts, and regression fixtures. |

---

### Task 1: Lock the YAML parser dependency and CI environment

**Files:**

- Create: `scripts/requirements-workflow-tests.txt`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/dependabot.yml`

**Interfaces:**

- Consumes: the Rust CI job's existing Python release-fixture invocation and
  the pinned `actions/setup-python` SHA already used by the release workflow.
- Produces: `python3` in CI with PyYAML 6.0.3 installed from an approved wheel
  before `scripts/test_release_workflow.py` runs.

- [ ] **Step 1: Create the exact hash-locked requirements file**

Create `scripts/requirements-workflow-tests.txt` with exactly this dependency
and the two CI wheel hashes:

```text
PyYAML==6.0.3 \
    --hash=sha256:ba1cc08a7ccde2d2ec775841541641e4548226580ab850948cbfda66a1befcdc \
    --hash=sha256:fc09d0aa354569bc501d4e787133afc08552722d3ab34836a80547331bb5d4a0
```

The first hash is the CPython 3.12 manylinux x86_64 wheel and the second is
the CPython 3.12 macOS arm64 wheel. Do not add an sdist hash; the installation
must remain binary-only.

- [ ] **Step 2: Prove the locked local install works before editing CI**

Run the exact command that CI will use:

```bash
python3 -m pip install \
  --disable-pip-version-check \
  --only-binary=:all: \
  --require-hashes \
  -r scripts/requirements-workflow-tests.txt
python3 -c 'import yaml; assert yaml.__version__ == "6.0.3"'
```

Expected: installation accepts one of the listed hashes and the assertion exits
zero. A changed hash or source-only installation must fail under
`--require-hashes --only-binary=:all:`.

- [ ] **Step 3: Set up and install Python in the Rust CI job**

In `.github/workflows/ci.yml`, add this SHA-pinned step immediately before the
existing `Validate release and installer fixtures` step in the `rust` job:

```yaml
      - name: Install Python 3.12 for workflow-contract tests
        uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7
        with:
          python-version: '3.12'

      - name: Install locked workflow-test dependencies
        run: >-
          python3 -m pip install --disable-pip-version-check --only-binary=:all:
          --require-hashes -r scripts/requirements-workflow-tests.txt
```

Keep the existing fixture command unchanged so the dependency is exercised by
both Rust matrix operating systems.

- [ ] **Step 4: Add pip maintenance coverage**

Append this update entry to `.github/dependabot.yml`, matching the existing
weekly schedule, open-pull-request limit, and `chore` commit prefix:

```yaml
  - package-ecosystem: pip
    directory: /
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
    commit-message:
      prefix: chore
```

- [ ] **Step 5: Validate configuration and commit**

Run:

```bash
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path) }' \
  .github/workflows/ci.yml .github/dependabot.yml
python3 -m pip install --disable-pip-version-check --only-binary=:all: \
  --require-hashes -r scripts/requirements-workflow-tests.txt
git diff --check
```

Commit only the three task files:

```bash
git add scripts/requirements-workflow-tests.txt .github/workflows/ci.yml .github/dependabot.yml
git commit -m "ci: lock YAML workflow test parser"
```

### Task 2: Parse workflow mappings and assert both attestation contracts

**Files:**

- Modify: `scripts/test_release_workflow.py`

**Interfaces:**

- Consumes: PyYAML 6.0.3 from `scripts/requirements-workflow-tests.txt` and
  the current release workflow at `.github/workflows/release.yml`.
- Produces: parsed action-reference checking and reusable parsed-step contract
  helpers used by `ReleaseWorkflowTests`.

- [ ] **Step 1: Write failing parser regression fixtures**

Replace the lexical-scanner-specific fixture expectations with parsed-YAML
fixtures that make each mutable reference fail:

```yaml
# normal-quoted.yml
jobs:
  test:
    steps:
      - 'uses': actions/checkout@v7

# flow-quoted.yaml
steps: [{ "uses": actions/checkout@v7 }]

# nested-quoted.yaml
jobs:
  test:
    steps:
      - nested:
          - uses: actions/checkout@v7
```

In the same test, include strings that must remain inert:

```yaml
note: '"uses": actions/checkout@v7'
run: |
  echo 'uses: actions/checkout@v7'
```

Add an invalid YAML fixture such as `jobs: [`. The test must expect a clear
`AssertionError` naming the invalid workflow path. Run the focused test before
the implementation; quoted-key cases must initially escape the old scanner.

- [ ] **Step 2: Replace the lexical scanner with parsed recursive traversal**

Import `yaml` and add focused helpers with these semantics:

```python
def load_workflow(path: Path) -> object:
    try:
        return yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as error:
        raise AssertionError(f"invalid workflow YAML: {path}") from error


def parsed_action_references(node: object) -> tuple[str, ...]:
    references: list[str] = []

    def visit(value: object) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                if key == "uses":
                    if not isinstance(child, str):
                        raise AssertionError("workflow uses value must be a string")
                    references.append(child)
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(node)
    return tuple(references)
```

Make `assert_actions_are_immutable()` load every `workflow_paths()` entry and
apply the existing full-SHA regular expression to every returned reference.
Retain `EXPECTED_ACTION_REFERENCES` as an exact raw-text assertion for the
current SHA plus adjacent version-comment review aid. Delete obsolete lexical
scanner helpers only after every caller uses the parsed helpers.

- [ ] **Step 3: Add parsed release-step contract helpers and tests**

Add helpers that select the parsed `jobs.build.steps` list by the exact step
name, returning the matching mapping or raising an assertion. Add a helper
that enforces the two attestations:

```python
def assert_release_attestations(build: dict[str, object]) -> None:
    provenance = named_step_mapping(build, "Attest release archive provenance")
    assert_step_value(provenance, "uses", "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6")
    assert_step_value(provenance, "with.subject-path", "${{ steps.package.outputs.archive }}")
    assert_missing_step_value(provenance, "with.sbom-path")

    sbom = named_step_mapping(build, "Attest release archive SBOM")
    assert_step_value(sbom, "uses", "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6")
    assert_step_value(sbom, "with.subject-path", "${{ steps.package.outputs.archive }}")
    assert_step_value(sbom, "with.sbom-path", "${{ steps.package.outputs.sbom }}")
```

Use concrete helper implementations rather than Python `assert` statements so
the checks remain active under optimization. In focused regression tests, use
`copy.deepcopy()` of the parsed build mapping and prove each of the following
mutations raises `AssertionError`: remove provenance `uses`, replace it with
`evil/actions/attest@0123456789abcdef0123456789abcdef01234567`, add an
unquoted `sbom-path`, remove the SBOM step, or change its `sbom-path`. Also
load a minimal release-like YAML fixture whose provenance `with` mapping uses
`"sbom-path"` as a quoted source key, then prove the parsed helper raises the
same `AssertionError` after YAML normalizes that key.

- [ ] **Step 4: Preserve raw tag-publication checks and run the full script**

Keep the existing raw-text shell assertions for `verify_release_tag()` and its
immediate placement before `gh release upload` and `gh release create`; these
assert shell sequencing rather than YAML structure. Run:

```bash
python3 scripts/test_release_workflow.py
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path) }' \
  .github/workflows/ci.yml .github/workflows/release.yml .github/workflows/skill-verify.yml
git diff --check
```

Expected: all workflow-contract tests pass, every workflow parses, and no
whitespace errors appear.

- [ ] **Step 5: Commit**

```bash
git add scripts/test_release_workflow.py
git commit -m "test: parse workflow action and attestation contracts"
```

### Task 3: Verify the parser gate across the public-readiness branch

**Files:**

- No planned file modifications. This task is read-only verification; a failed
  gate returns work to the owning Task 1 or Task 2 implementation scope.

**Interfaces:**

- Consumes: locked parser installation, parsed workflow contract test, the
  public-readiness branch, and the repository documentation-style checks.
- Produces: fresh merge evidence for the PR and all dependent GitHub settings.

- [ ] **Step 1: Run local parser gate and dependency proof**

```bash
python3 -m pip install --disable-pip-version-check --only-binary=:all: \
  --require-hashes -r scripts/requirements-workflow-tests.txt
python3 scripts/test_release_workflow.py
```

Expected: PyYAML resolves to 6.0.3 from a listed hash and every parser/contract
test passes.

- [ ] **Step 2: Run repository build and release-fixture gates**

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
npm --prefix apps/skilltape-console test
python3 scripts/test_release_package.py
bash scripts/test_install.sh
```

Expected: all commands exit zero; existing Console color-environment warnings
may be recorded but do not hide a test failure.

- [ ] **Step 3: Run YAML, English, Markdown-link, and whitespace gates**

```bash
ruby -e 'require "yaml"; Dir[".github/**/*.yml", ".github/**/*.yaml"].each { |path| YAML.load_file(path); puts "YAML OK #{path}" }'
if git grep -I -n --perl-regexp '[\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}]' -- . ':!Cargo.lock'; then
  exit 1
fi
python3 - <<'PY'
from pathlib import Path
import re
import subprocess
import sys

pattern = re.compile(r'(?<!!)\[[^\]]+\]\(([^)\s]+)(?:\s+["\'][^"\']*["\'])?\)')
missing = []
for raw in subprocess.check_output(["git", "ls-files", "-z", "--", "*.md"]).split(b"\0"):
    if not raw:
        continue
    source = Path(raw.decode())
    for match in pattern.finditer(source.read_text(encoding="utf-8")):
        target = match.group(1).strip("<>")
        if not target or target.startswith("#") or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", target):
            continue
        local = target.split("#", 1)[0].split("?", 1)[0]
        if local and not (source.parent / local).exists():
            missing.append(f"{source}: {target}")
if missing:
    print("Missing Markdown targets:", file=sys.stderr)
    print("\n".join(missing), file=sys.stderr)
    raise SystemExit(1)
PY
git diff --check
```

Expected: the text-only language scan emits no match, every tracked Markdown
relative target exists, and the working tree has no whitespace errors.

- [ ] **Step 4: Request fresh whole-branch review and commit only defect fixes**

Build a review package from `origin/main` to `HEAD` and request a read-only
review focused on parser dependency integrity, semantic action traversal,
provenance/SBOM contract coverage, release tag revalidation, public security
guidance, and public documentation claims. Resolve every Critical or Important
finding with a new focused test before creating the pull request. Record only
verified passing evidence; do not create a release or tag during review.
