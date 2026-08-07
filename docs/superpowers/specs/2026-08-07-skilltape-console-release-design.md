# SkillTape Console Release Design

## Status

Approved design for the M3 release-assets milestone. The implementation is
limited to packaging, installation, discovery, and release verification. It
does not change the read-only Console API contract or add provider execution.

## Goals

1. A release archive must contain everything required by `skilltape console`:
   the CLI binary, the `skilltape-console-api` companion binary, and the built
   static UI.
2. Unix and Windows installation scripts must install all three assets without
   replacing an existing CLI when download, checksum, extraction, or staging
   fails.
3. The CLI must discover the companion binary and UI from the installed layout
   while retaining the existing `SKILLTAPE_CONSOLE_API_BIN` and
   `SKILLTAPE_CONSOLE_UI_DIST` overrides.
4. Tag-driven GitHub Actions must build locked Rust and npm dependencies,
   produce target-named archives, and publish a checksums file.
5. Release verification must prove archive completeness, checksum correctness,
   Console API startup, static UI serving, and loopback-only binding.

## Non-goals

- Adding a remote model provider or changing the offline compiler policy.
- Making Replay/Verify available on Windows without an equivalent sandbox.
- Uploading Tape, Receipt, logs, environment variables, or credentials.
- Changing the Console API schemas, read model, or browser information
  architecture.

## Release layout

Each archive has a single top-level directory named
`skilltape-v<version>-<target>/`:

```text
skilltape-v<version>-<target>/
├── skilltape
├── skilltape-console-api
└── console/
    ├── index.html
    └── assets/
```

Windows uses `.exe` suffixes and a `.zip` archive. Unix targets use `.tar.gz`.
The UI directory is deliberately named `console` so an installation can place
it beside the `bin` directory without recreating the source repository tree.

The install scripts copy the two binaries into the selected install directory
and copy `console/` into a sibling `console/` directory. For the default
`$HOME/.local/bin` layout this becomes:

```text
$HOME/.local/bin/skilltape
$HOME/.local/bin/skilltape-console-api
$HOME/.local/console/index.html
```

The Windows default follows the same relationship under the selected install
directory's parent. Existing binaries are replaced only after all assets have
been downloaded, verified, extracted, and staged.

## Runtime discovery

`skilltape console` keeps environment-variable overrides as the highest
priority. Without overrides it searches:

1. The source/development layout (`apps/skilltape-console/dist` and nearby API
   binary).
2. The installed layout (`console/` next to the install directory and the API
   binary beside the CLI).
3. Directories on `PATH` for the API companion.

Every candidate is checked as a non-symlink regular directory/file and the UI
must contain a regular `index.html`. The API remains bound to `127.0.0.1` by
the CLI. The API's existing static path and workspace symlink checks remain in
force.

## Build and release workflow

The release workflow runs only for version tags matching `v*` or by an explicit
manual dispatch with a version. A build job runs per target, using the pinned
Rust toolchain and `cargo build --locked --release`; the UI is installed with
`npm ci` and built with the checked-in npm lockfile. A packaging script creates
the archive and validates that the CLI, API companion, `console/index.html`,
and at least one hashed UI asset are present.

The initial target matrix is:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

Each archive is uploaded as a workflow artifact. A final release job downloads
the artifacts, generates `checksums.txt` from the exact archive bytes, and
publishes the assets to the tag release. The workflow has `contents: write`
only on this final job; build jobs retain read-only permissions. No secret is
passed to build or test commands.

## Verification strategy

The release gates are layered:

1. Rust formatting, Clippy, locked workspace tests, and the existing product
   security/journey tests.
2. `npm ci`, TypeScript/Vite production build, and Playwright browser tests.
3. A package-layout test that checks both binaries, the UI entrypoint, and the
   checksum input before an archive is accepted.
4. A Console API smoke test that starts the packaged API against a temporary
   workspace, requests `/api/v1/workspaces` and `/`, and confirms that the
   packaged CLI path uses loopback only. Direct API invocation may still emit
   its existing warning when explicitly configured with a non-loopback bind.
5. Installer tests using a local file fixture to verify checksum mismatch and
   incomplete archive failures preserve the previous CLI binary.

The current local macOS environment can run the Rust and UI gates. Linux
`bubblewrap` and Windows sandbox limitations remain explicit environment
constraints for Replay/Verify and are not hidden by the release workflow.

## Failure handling and risks

- Missing API or UI assets fail before the CLI starts a child process.
- Any checksum, extraction, or staging error leaves the destination binaries
  unchanged.
- The repository currently has no configured Git remote; release publishing is
  therefore implemented as a workflow definition and must be enabled only
  after a remote and GitHub release policy are supplied.
- Cross-compiling the Windows target and installing Playwright browsers are
  CI-only checks; local macOS verification must not be presented as proof for
  those targets.

## Acceptance criteria

- A locally built archive passes the package-layout and checksum checks.
- An installed archive starts `skilltape console` with no source checkout and
  serves the built UI through the loopback API process.
- `npm ci` and all locked Rust commands pass from a clean dependency state.
- The release workflow has no artifact upload for Tape, Receipt, logs, or
  secrets and grants write permissions only to the final publishing job.
