# SkillTape

SkillTape is a local-first, replay-verifiable Agent Skill compiler. It records
real terminal and filesystem workflows as redactable, reviewable Tapes, then
deterministically compiles them into replayable, verifiable Skill packages that
can be committed to GitHub.

The core loop is:

```text
Capture → Tape → Compile → Lint/Policy → Replay → Verify/Receipt → Export
```

The core runtime does not require a cloud service or model provider. A model
proposal may supplement descriptions, but it cannot bypass schema, permission,
or policy gates.

## Five-minute local run

Rust 1.97.1 is required. Replay/Verify on Linux also requires `bubblewrap`,
while macOS uses the system `/usr/bin/sandbox-exec`. See the
[installation guide](docs/guides/installation.md) for source builds and
prebuilt release installation.

```bash
git clone <your-skilltape-repository>
cd skilltape

# Build the CLI; core commands do not require Node.js or cloud services
cargo install --locked --path crates/skilltape-cli

demo_workspace="$(mktemp -d)"
skilltape capture demo \
  --workspace "$demo_workspace" \
  --command /bin/echo \
  --output "$demo_workspace/.skilltape/tapes/tape_demo" \
  --yes
skilltape compile "$demo_workspace/.skilltape/tapes/tape_demo" \
  --output "$demo_workspace/demo-skill"
skilltape lint "$demo_workspace/demo-skill"
skilltape verify "$demo_workspace/demo-skill" --json
skilltape export "$demo_workspace/demo-skill" \
  --target generic \
  --output "$demo_workspace/exported-skill"
```

`capture --yes` is explicit local confirmation. Capture does not persist raw
secret environment-variable values by default, and Tape output should still be
treated as a sensitive local artifact. Before committing to a repository,
inspect `.gitignore`, Tapes, Receipts, and export directories.

To record a human interaction, omit `--command` and Capture starts the current
user's shell. You can also append `--interactive` to a specified program. Live
PTY output during an interactive session is written to stderr, so stdout
remains a single JSON summary when used with `--json`. Every captured Tape
manifest uses an independent ID; IDs are not reused even when output
directories differ but names match.

## Console

Console is an optional read-only local viewer for Capture timelines,
Workflow/permission diffs, run status, and Receipts. It does not execute
commands in the browser or modify the workspace.

Running Console from source also requires building the UI and API companion
binary:

```bash
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
cargo build --locked --release -p skilltape-cli -p skilltape-console-api

./target/release/skilltape console --workspace .
```

The CLI binds to `127.0.0.1` by default and reclaims the API child process on
exit; append `--open` to open a browser automatically. If the UI has not been
built, Console reports an explicit error instead of pretending to have started.

Release archives include `skilltape`, `skilltape-console-api`, and the static
`console/` UI. The installer places both binaries in the installation directory
and `console/` in its parent; after installation, `skilltape console` runs
without a source checkout. To override automatic discovery, set
`SKILLTAPE_CONSOLE_API_BIN` or `SKILLTAPE_CONSOLE_UI_DIST`.

## CLI commands

| Command | Purpose |
| --- | --- |
| `skilltape init <name> --output <dir>` | Create a minimal Skill package template |
| `skilltape lint <skill> [--strict] [--json]` | Validate schema, paths, permissions, policy, and lockfile |
| `skilltape capture <name> [--workspace <dir>] [--command <program>] [--interactive] --yes` | Record terminal and filesystem changes as a Tape; omit `--command` to enter the current shell, or append `--interactive` when the specified program reads from the terminal |
| `skilltape compile <tape> --output <dir>` | Deterministically compile a Skill package without a model |
| `skilltape replay <skill> [--input <json>]` | Replay in an isolated temporary workspace and output a redacted summary |
| `skilltape verify <skill> [--receipt <json>] [--json]` | Replay, run assertions, and generate a Receipt |
| `skilltape export <skill> --target <target> --output <dir>` | Export a generic or platform package through the lint gate |
| `skilltape console [--workspace <dir>] [--port <port>] [--open]` | Start the read-only local Console |

Every command can be run from source with `cargo run -p skilltape-cli -- ...`.
Invalid input usually returns code 2; policy/export/verify failures return a
non-zero code. CI should assert failures rather than ignore them.

## CI and Skill repository integration

- `.github/workflows/ci.yml` runs fmt, Clippy, workspace tests, valid-example lint, expected-failure assertions for invalid fixtures, and release/installer fixture gates.
- `.github/workflows/release.yml` builds Linux, macOS, and Windows archives on a `v*` tag or manual version input, generates `checksums.txt`, and grants `contents: write` only to the final publish job.
- `.github/workflows/skill-verify.yml` is a template that runs only the local CLI; it does not upload Tapes, Receipts, logs, or secrets.
- The release installation script requires a fixed version, downloads a checksum, and does not replace an existing binary until verification succeeds; see the [installation guide](docs/guides/installation.md) for the exact parameters.
- The implementation and release workflow are merged on `main` at `beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6`; final release run `31167200699` is green, and [SkillTape v0.1.0](https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0) is published with four verified archives and `checksums.txt`.

## Security, compatibility, and benchmarks

- [Security model and vulnerability disclosure](SECURITY.md) explains the sandbox boundary, secret handling, platform differences, and version policy.
- [Installation and platform prerequisites](docs/guides/installation.md): Linux Replay/Verify requires `bubblewrap`, while macOS uses `/usr/bin/sandbox-exec`.
- `cargo bench -p skilltape-cli --bench capture_compile` provides 10k Tape events, a 100-step Workflow, and an optional 1GB sparse-log scenario; it reports observations only and does not block functional tests with an uncalibrated fixed threshold. Set `SKILLTAPE_BENCHMARK_LARGE=1` for the large-log scenario.

The complete product CI gates cover Linux and macOS. Windows currently
supports Capture/Compile/Lint/Export; Replay/Verify fails closed until a future
equivalent restricted executor is integrated.

## Design goals

- Local-first operation without a mandatory cloud service.
- LLMs may generate constrained structured Workflow IR, but cannot execute arbitrary Shell directly.
- Deny undeclared file, network, process, and secret access by default.
- Use fixtures, controlled replay, and Receipts to prove Skill execution results.
- Connect different Agent platforms through Git, JSON/YAML, and adapters.

## Documentation

- [Documentation index](docs/README.md) — audience-oriented paths through the complete documentation set.
- [Installation, releases, and GitHub Actions](docs/guides/installation.md)
- [Release readiness checklist](docs/release-readiness.md)
- [Contributing guide](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)
- [Complete product design](docs/superpowers/specs/2026-08-05-skilltape-full-product-design.md)
- [Implementation plan](docs/superpowers/plans/2026-08-05-skilltape-full-product.md)
