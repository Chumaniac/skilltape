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

The installer requires an explicit HTTPS release download root so it cannot
silently select a different project. For the public SkillTape repository, use
the following runnable example:

```bash
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download"
SKILLTAPE_VERSION=0.1.0 ./scripts/install.sh
```

Keep `SKILLTAPE_RELEASE_BASE_URL` explicit when using a trusted mirror or
private release: replace only its value with that release's HTTPS download
root, and keep the version and target fixed.

The version can also be passed as the first argument, with the installation
directory and target overridden:

```bash
SKILLTAPE_RELEASE_BASE_URL="https://github.com/Chumaniac/skilltape/releases/download" \
  ./scripts/install.sh 0.1.0 "$HOME/.local/bin" "aarch64-apple-darwin"
```

Windows PowerShell:

```powershell
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/Chumaniac/skilltape/releases/download"
$env:SKILLTAPE_VERSION = "0.1.0"
.\scripts\install.ps1
```

The [release workflow](../../.github/workflows/release.yml) is triggered by a
`v*` tag or manual version input and covers Linux x86_64, macOS x86_64/arm64,
and Windows x86_64. The workflow does not upload Tapes, Receipts, logs,
environment variables, or provider credentials. [SkillTape v0.1.0](https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0)
is the published checksum-verified release and includes the four target
archives plus `checksums.txt`. It was published before provenance and SBOM
generation was added, so this historical release has no archive-local SBOM or
GitHub attestation claim.

The script downloads the archive and `checksums.txt` into a random temporary
directory, compares the target asset's SHA-256, validates the CLI, API
companion, and Console UI in the archive, and replaces the target files only
after all assets have been staged. A download failure, checksum mismatch,
missing archive asset, or permission failure leaves the existing binary
untouched. The version, download root, and target can all be fixed; the script
does not read or write tokens, cookies, environment secrets, or the project's
`.env`.

## Verify future release integrity

Future release workflows require an existing matching `v<version>` tag whose
commit resolves to the workflow commit. For each target, the workflow publishes
the release archive, an archive-local SPDX JSON sidecar named
`<archive>.spdx.json`, GitHub artifact attestations for build provenance and
the SPDX predicate, and SHA-256 entries in `checksums.txt` for the archive and
its sidecar. These are future-release requirements; they do not change the
historical evidence for `v0.1.0`.

After downloading a future archive and its adjacent SPDX sidecar into the
current directory, set its version and target (the values below are an
example), then download the matching `checksums.txt`:

```bash
set -e
version=0.2.0
target=x86_64-unknown-linux-gnu
archive="skilltape-v${version}-${target}.tar.gz"
sbom="${archive}.spdx.json"
test -f "$archive" && test -f "$sbom"

curl --fail --location --silent --show-error \
  --output checksums.txt \
  "https://github.com/Chumaniac/skilltape/releases/download/v${version}/checksums.txt"
awk -v asset="$archive" '$2 == asset { print }' checksums.txt | sha256sum -c -
awk -v asset="$sbom" '$2 == asset { print }' checksums.txt | sha256sum -c -

gh attestation verify "$archive" \
  --repo Chumaniac/skilltape
gh attestation verify "$archive" \
  --repo Chumaniac/skilltape \
  --predicate-type https://spdx.dev/Document/v2.3 \
  --format json \
  --jq '.[].verificationResult.statement.predicate'
```

After both checksum commands succeed, the sidecar is the human-downloadable
SBOM document. The first attestation command verifies the archive's default
build-provenance claim; the second verifies the SPDX predicate attached to the
archive before you rely on the sidecar's contents.

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
