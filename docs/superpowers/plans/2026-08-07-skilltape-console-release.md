# SkillTape Console Release Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Produce verified release archives and installers containing the SkillTape CLI, Console API companion, and static Console UI.

**Architecture:** Keep the existing read-only API and CLI process supervisor unchanged except for installed-asset discovery. A dependency-free Python packager assembles target-specific archives; the existing shell and PowerShell installers validate and stage all three assets. A tag-driven GitHub Actions workflow builds locked dependencies, uploads per-target artifacts, and publishes checksums and archives in one write-permission job.

**Tech Stack:** Rust 1.97.1, Cargo.lock, Node.js 22, npm lockfile, Vite/Playwright, Python 3 standard library for archive assembly, Bash, PowerShell, GitHub Actions.

## Global Constraints

- Archive root is skilltape-v<version>-<target>/ and contains skilltape, skilltape-console-api, and console/index.html.
- Unix archives are .tar.gz; Windows archives are .zip; Windows binaries use .exe.
- Installed layout places both binaries in the selected install directory and console/ in its parent directory.
- SKILLTAPE_CONSOLE_API_BIN and SKILLTAPE_CONSOLE_UI_DIST remain highest-priority overrides.
- Rust commands use the repository-pinned Rust 1.97.1 toolchain and --locked; UI commands use npm ci.
- Release build jobs have contents: read; only the final publisher has contents: write.
- Release jobs must not upload Tape, Receipt, logs, environment values, or provider credentials.
- CLI-launched Console binds to 127.0.0.1; static files and workspace resources reject symlink escapes.

## File Map

- Modify crates/skilltape-cli/src/console_command.rs for installed sibling console discovery and pure candidate tests.
- Modify crates/skilltape-cli/tests/console_command.rs for the installed discovery contract.
- Create scripts/package_release.py as the cross-platform archive assembler and validator.
- Create scripts/test_release_package.py for archive completeness and symlink rejection.
- Create scripts/smoke_console.sh for actual packaged API/UI HTTP verification.
- Modify scripts/install.sh and scripts/install.ps1 to install the API companion and static UI.
- Create scripts/test_install.sh for local-fixture installer regression checks.
- Create .github/workflows/release.yml for target builds and publication.
- Create scripts/test_release_workflow.py for static workflow security checks.
- Modify README.md and docs/guides/installation.md with packaged Console use and release verification.

---

### Task 1: Installed Console asset discovery

**Files:**
- Modify: crates/skilltape-cli/src/console_command.rs
- Test: crates/skilltape-cli/tests/console_command.rs

**Interfaces:**
- Preserve resolve_ui_dist() -> Result<PathBuf, ConsoleError> and resolve_api_binary() -> Result<PathBuf, ConsoleError>.
- Add a private helper accepting optional current directory/executable paths and returning ordered UI candidates, so tests do not mutate process-wide environment variables.

- [ ] Step 1: Write the failing discovery test

Create an installed-layout fixture with bin/skilltape, bin/skilltape-console-api, and console/index.html. Assert the candidate resolver returns the sibling console directory when source checkout candidates are absent.

- [ ] Step 2: Run the focused test and verify it fails

~~~
cargo test --locked -p skilltape-cli --bin skilltape console_command -- --nocapture
~~~

Expected: FAIL because the resolver does not include the installed sibling console directory.

- [ ] Step 3: Implement the smallest discovery change

Add ancestor.join("console") to the executable-relative UI candidates after the existing source-layout candidates. Keep environment overrides first, preserve non-symlink/index validation, and do not broaden API binary validation.

- [ ] Step 4: Run focused and existing Console tests

~~~
cargo test --locked -p skilltape-cli --test console_command -- --test-threads=1
cargo clippy --locked -p skilltape-cli --all-targets -- -D warnings
~~~

Expected: PASS with no new warnings.

- [ ] Step 5: Commit

~~~
git add crates/skilltape-cli/src/console_command.rs crates/skilltape-cli/tests/console_command.rs
git commit -m "feat: discover installed Console assets"
~~~

### Task 2: Cross-platform release packager

**Files:**
- Create: scripts/package_release.py
- Create: scripts/test_release_package.py

**Interfaces:**
- Command: python3 scripts/package_release.py --version VERSION --target TARGET --binary-dir DIR --ui-dist DIR --output-dir DIR.
- Output: exactly one skilltape-vVERSION-TARGET.tar.gz for Unix targets or .zip for Windows targets.
- The script exits nonzero unless both binaries, console/index.html, and at least one regular console/assets/* file exist and are not symlinks.

- [ ] Step 1: Write the failing package test

Create temporary fake release binaries and a minimal UI fixture. Run the packager and assert the archive contains the exact top-level paths skilltape-v0.1.0-test/skilltape, skilltape-v0.1.0-test/skilltape-console-api, skilltape-v0.1.0-test/console/index.html, and one asset. Add a second case omitting the API binary and assert a nonzero exit without an archive.

- [ ] Step 2: Run the test and verify it fails

~~~
python3 scripts/test_release_package.py
~~~

Expected: FAIL because scripts/package_release.py does not exist.

- [ ] Step 3: Implement the packager

Use only argparse, pathlib, shutil, tempfile, tarfile, and zipfile. Validate version and target path components, reject symlink inputs, stage under a temporary directory, write the target archive, and remove any partial output on failure. Use .exe names and ZIP for targets containing windows.

- [ ] Step 4: Run package tests and Python safety checks

~~~
python3 scripts/test_release_package.py
python3 -m py_compile scripts/package_release.py scripts/test_release_package.py
~~~

Expected: PASS; incomplete or symlinked fixtures leave no output archive.

- [ ] Step 5: Commit

~~~
git add scripts/package_release.py scripts/test_release_package.py
git commit -m "build: add cross-platform Console release packager"
~~~

### Task 3: Install all release assets safely

**Files:**
- Modify: scripts/install.sh
- Modify: scripts/install.ps1
- Create: scripts/test_install.sh

**Interfaces:**
- Existing version/base URL/target environment variables remain unchanged.
- Unix installation writes <install-dir>/skilltape, <install-dir>/skilltape-console-api, and <parent-of-install-dir>/console/.
- Windows installation writes the equivalent .exe files and sibling console/ directory.

- [ ] Step 1: Write the failing local-fixture installer test

Create a temporary release root served by python3 -m http.server, generate a valid package with Task 2, seed an existing CLI marker, and run scripts/install.sh. Assert both binaries and console/index.html are installed. Then corrupt checksums.txt, rerun against a second install directory containing an existing marker, and assert the marker is unchanged and no new API/UI files are installed.

- [ ] Step 2: Run the test and verify it fails

~~~
bash scripts/test_install.sh
~~~

Expected: FAIL because the installer currently copies only skilltape.

- [ ] Step 3: Update the Unix installer

After checksum verification, require regular non-symlink CLI/API/UI inputs, copy all assets into temporary staging paths, validate staged files, then replace the two binaries and sibling UI directory. Preserve the existing temporary-directory cleanup and HTTPS download restrictions.

- [ ] Step 4: Update the PowerShell installer

Mirror the Unix validation and staging behavior with Expand-Archive, Get-FileHash, Copy-Item, and a staged sibling UI directory. Reject missing API/UI assets before replacing the existing CLI.

- [ ] Step 5: Run installer and static checks

~~~
bash scripts/test_install.sh
bash -n scripts/install.sh scripts/test_install.sh
~~~

Expected: PASS. PowerShell syntax is checked in Windows CI with a local fixture.

- [ ] Step 6: Commit

~~~
git add scripts/install.sh scripts/install.ps1 scripts/test_install.sh
git commit -m "feat: install Console companion assets"
~~~

### Task 4: Packaged Console API/UI smoke verification

**Files:**
- Create: scripts/smoke_console.sh

**Interfaces:**
- Command: SKILLTAPE_CONSOLE_API_BIN=... SKILLTAPE_CONSOLE_UI_DIST=... bash scripts/smoke_console.sh WORKSPACE.
- The script starts the actual API binary on a loopback ephemeral port, polls readiness, fetches /api/v1/workspaces and /, checks JSON/HTML content, and always terminates the child.

- [ ] Step 1: Write the failing smoke harness

Add the script against the current API binary and built UI, with a temporary empty workspace. Assert the workspace response has skilltape.dev/console/v1 and the root response contains the Console HTML title.

- [ ] Step 2: Run it to expose the first missing precondition

~~~
SKILLTAPE_CONSOLE_API_BIN=target/debug/skilltape-console-api
SKILLTAPE_CONSOLE_UI_DIST=apps/skilltape-console/dist
bash scripts/smoke_console.sh "$(mktemp -d)"
~~~

Expected before build: a clear missing-binary or missing-UI failure; after the implementation and build, PASS.

- [ ] Step 3: Implement bounded readiness, HTTP checks, and cleanup

Use Python standard-library urllib.request for HTTP and a shell trap to kill/wait for the child. Use a fixed short deadline and fail if the process exits before the ready line. Do not send workspace contents outside the local process.

- [ ] Step 4: Run the complete smoke path

~~~
cargo build --locked -p skilltape-console-api
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
SKILLTAPE_CONSOLE_API_BIN=target/debug/skilltape-console-api
SKILLTAPE_CONSOLE_UI_DIST=apps/skilltape-console/dist
bash scripts/smoke_console.sh "$(mktemp -d)"
~~~

Expected: PASS and child cleanup after completion.

- [ ] Step 5: Commit

~~~
git add scripts/smoke_console.sh
git commit -m "test: add packaged Console smoke gate"
~~~

### Task 5: Tag-driven release workflow

**Files:**
- Create: .github/workflows/release.yml
- Create: scripts/test_release_workflow.py

**Interfaces:**
- Push trigger: tags matching v*.
- Manual trigger: required version input without a leading v.
- Build matrix targets: Linux x86_64, macOS x86_64, macOS arm64, Windows x86_64.
- Publisher uploads all archives and checksums.txt to release tag vVERSION.

- [ ] Step 1: Write workflow validation expectations

Add a repository-local validation command that reads the workflow text and asserts the workflow contains locked Rust/npm steps, all four targets, artifact upload, checksum generation, and contents: write only on the publish job. Keep YAML parsing as a separate Ruby CI check so the validator has no third-party Python dependency.

- [ ] Step 2: Run validation and verify it fails

~~~
python3 scripts/test_release_workflow.py
~~~

Expected: FAIL because .github/workflows/release.yml and its validator do not exist.

- [ ] Step 3: Implement build matrix and packaging

Use dtolnay/rust-toolchain@master with toolchain: 1.97.1, actions/setup-node@v4 with Node 22, actions/setup-python@v5, npm ci, locked release builds for both Rust binaries, the package script, and actions/upload-artifact@v4. Keep build permissions read-only.

- [ ] Step 4: Implement final checksum/release publishing

Use an Ubuntu publisher job with contents: write, download all artifacts, generate sorted SHA-256 entries, and use the built-in GitHub token through gh release create/release upload. Do not upload workspace or test output.

- [ ] Step 5: Run workflow validation and YAML parsing

~~~
python3 scripts/test_release_workflow.py
ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml")'
~~~

Expected: PASS.

- [ ] Step 6: Commit

~~~
git add .github/workflows/release.yml scripts/test_release_workflow.py
git commit -m "ci: add target release workflow"
~~~

### Task 6: Documentation and full release gates

**Files:**
- Modify: README.md
- Modify: docs/guides/installation.md
- Modify: .github/workflows/ci.yml only if the new scripts need an explicit gate

- [ ] Step 1: Document packaged Console use

Add archive contents, installed paths, API/UI environment override troubleshooting, and exact checksum/install commands. Use the [installation guide](../../guides/installation.md) for package and installation details, link current status to the [release-readiness page](../../release-readiness.md), and link the [release workflow](../../../.github/workflows/release.yml) without changing its `v*` tag/manual-dispatch triggers or publication behavior.

- [ ] Step 2: Run the complete local gate

~~~
export PATH="/Users/chumanic/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
npm --prefix apps/skilltape-console test
python3 scripts/test_release_package.py
python3 scripts/test_release_workflow.py
bash scripts/test_install.sh
~~~

- [ ] Step 3: Inspect final scope and status

~~~
git diff --check
git status --short
git log -5 --oneline
~~~

Expected: only the pre-existing SDD report edits remain unstaged; all M3 code and tests are committed.

- [ ] Step 4: Commit documentation

~~~
git add README.md docs/guides/installation.md
git commit -m "docs: document packaged Console installation"
~~~
