# Installation and local CI

SkillTape can be installed in two ways: build it from source, or download a
verified binary for the target platform from a GitHub Release. The core CLI
does not require a cloud service; Console requires both the
`skilltape-console-api` binary and a built Vite `dist` directory.

Use the [documentation index](../README.md) to choose a path by audience and
the [guides index](README.md) for related operational guidance.

## Build from source

After installing Rust 1.97.1 (the repository's `rust-toolchain.toml` selects
this version automatically), run the following from the repository root:

```bash
cargo build --locked --release -p skilltape-cli -p skilltape-console-api
cargo install --locked --path crates/skilltape-cli
```

Replay/Verify also starts a restricted local executor: Linux requires `bwrap`
(Debian/Ubuntu can install `bubblewrap`), while macOS requires the system
`/usr/bin/sandbox-exec`. Without the corresponding sandbox, Capture, Compile,
Lint, and Export remain available, but Replay/Verify fails closed with an
environment-unavailable message.

Installing `skilltape` is sufficient when using only Capture, Compile, Lint,
Replay, Verify, or Export. To start Console from source:

```bash
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
./target/release/skilltape console --workspace .
```

During development, `npm --prefix apps/skilltape-console run dev` can be used
to view the UI, but the API must still be provided by `skilltape-console-api`
or `skilltape console`; the browser has no execution or write capability.

## Release installation

Release assets use the following names:

```text
skilltape-v<version>-<target>.tar.gz   # macOS/Linux
skilltape-v<version>-<target>.zip      # Windows
checksums.txt
```

Each archive contains:

```text
skilltape-v<version>-<target>/
├── skilltape
├── skilltape-console-api
└── console/
    ├── index.html
    └── assets/
```

After installation, the CLI and API companion are in the installation
directory, and `console/` is in the parent directory. For example, the default
Unix paths are `$HOME/.local/bin/skilltape`,
`$HOME/.local/bin/skilltape-console-api`, and
`$HOME/.local/console/index.html`. The installer downloads and verifies the
checksum, checks all three asset classes, and stages them before replacement;
download, verification, extraction, or staging failures do not overwrite the
existing CLI.

The current installer requires an explicit release download root to avoid
downloading from the wrong project while the repository owner, mirror, or
private release is not determined:

```bash
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/<owner>/<repo>/releases/download"
SKILLTAPE_VERSION=0.1.0 ./scripts/install.sh
```

The version can also be passed as the first argument, with the installation
directory and target overridden:

```bash
SKILLTAPE_RELEASE_BASE_URL="https://github.com/<owner>/<repo>/releases/download" \
  ./scripts/install.sh 0.1.0 "$HOME/.local/bin" "aarch64-apple-darwin"
```

Windows PowerShell:

```powershell
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/<owner>/<repo>/releases/download"
$env:SKILLTAPE_VERSION = "0.1.0"
.\scripts\install.ps1
```

The [release workflow](../../.github/workflows/release.yml) is triggered by a
`v*` tag or manual version input and covers Linux x86_64, macOS x86_64/arm64,
and Windows x86_64. The workflow does not upload Tapes, Receipts, logs,
environment variables, or provider credentials. No versioned GitHub Release
has been published yet; release publication requires the remaining release
checks and explicit release-tag approval.

The script downloads the archive and `checksums.txt` into a random temporary
directory, compares the target asset's SHA-256, validates the CLI, API
companion, and Console UI in the archive, and replaces the target files only
after all assets have been staged. A download failure, checksum mismatch,
missing archive asset, or permission failure leaves the existing binary
untouched. The version, download root, and target can all be fixed; the script
does not read or write tokens, cookies, environment secrets, or the project's
`.env`.

## Capture → Compile → Verify

The following example uses a temporary workspace so that Tapes and Receipts are
not written into the repository:

```bash
demo_workspace="$(mktemp -d)"
skilltape capture demo \
  --workspace "$demo_workspace" \
  --command /bin/echo \
  --output "$demo_workspace/.skilltape/tapes/tape_demo" \
  --yes
skilltape compile "$demo_workspace/.skilltape/tapes/tape_demo" \
  --output "$demo_workspace/demo-skill"
skilltape lint "$demo_workspace/demo-skill" --strict
skilltape verify "$demo_workspace/demo-skill" \
  --receipt "$demo_workspace/receipt.json" \
  --json
skilltape export "$demo_workspace/demo-skill" \
  --target generic \
  --output "$demo_workspace/exported-skill"
```

`--command` accepts an executable name. When arguments are needed, wrap them in
a controlled script and declare them in the Skill's permissions/workflow. Do
not concatenate unreviewed natural language directly into a Shell command.

To capture a human-operated workflow, omit `--command`; Capture starts the
current user's shell and ends after `exit` is entered. If the specified program
itself reads terminal input, add `--interactive`. Interactive mode sends live
terminal output to stderr, keeping the `--json` stdout summary clean. The Tape
manifest `id` is unique for every run.

## Local verification

The local gates matching CI (using the lockfiles and rejecting implicit
upgrades) are:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
cargo run --locked -p skilltape-cli -- lint examples/minimal-skill
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
npm --prefix apps/skilltape-console test
python3 scripts/test_release_package.py
python3 scripts/test_release_workflow.py
bash scripts/test_install.sh
```

An invalid fixture must fail explicitly (the current policy code is 3):

```bash
if cargo run --locked -p skilltape-cli -- lint tests/fixtures/invalid-skill; then
  echo "invalid fixture unexpectedly passed" >&2
  exit 1
fi
```

## GitHub Actions

When copying or enabling `.github/workflows/skill-verify.yml`, set `skill_path`
to an already reviewed Skill directory inside the repository. The template
checks out only the current repository, builds the local CLI, and runs `lint`;
it has no artifact upload, Tape/Receipt upload, remote provider, or secret-dump
step. If CI must generate a Receipt, keep it in the runner's temporary
directory and explicitly clean it up unless the repository has another reviewed
release policy.
