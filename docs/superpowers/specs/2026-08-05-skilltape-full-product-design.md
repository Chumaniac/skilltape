# SkillTape Full Product Design Specification

> Status: Awaiting user review
> Date: 2026-08-05
> Product form: local-first CLI/SDK at the core, with an optional local Web Console companion

> Archival note: This pre-migration specification preserves its historical status, dates, task evidence, commands, and intent. Current implementation and release facts are tracked in [docs/release-readiness.md](../../release-readiness.md).

## 1. Product Definition

SkillTape is a local-first toolchain that turns “a real piece of work completed by a person” into an “auditable, replayable, shareable Agent Skill.”

The core loop of the full product is:

```text
Capture → Tape → Compile → Policy/Lint → Replay → Verify/Receipt → Export
```

Core promise:

> One real operation produces an Agent Skill with evidence that can be replayed and submitted to GitHub.

SkillTape’s core capabilities must run locally, without a cloud account or fixed model provider. A model may only help interpret Tape and propose a structured Workflow; model output must not bypass the Schema, permission policy, or controlled Runner.

## 2. Product Boundaries

### 2.1 Included in the Full Product

- Rust CLI/SDK: installation, capture, compile, review, replay, verification, and export.
- Tape: a structured record of terminal activity, file changes, environment context, and permission decisions.
- Compiler: a compiler from Tape to Workflow IR, Skill documentation, and a permission manifest.
- Policy Engine: policies for paths, processes, networks, environment variables, and secret access.
- Replay Runner: temporary workspaces, process supervision, timeouts, cancellation, and output capture.
- Verify: fixtures, assertions, replay comparison, and Receipt generation.
- Export Adapter: generic Skill packages and a small number of high-value Agent platform adapters.
- Local Console: an optional timeline, Diff, permission review, and Receipt viewer.
- GitHub-ready project structure: directly reviewable, forkable, testable, and publishable.

### 2.2 Explicit Non-goals

- No cloud Skill marketplace, accounts, or billing system.
- SkillTape is not a general-purpose desktop RPA or arbitrary GUI automation tool.
- It does not allow arbitrary `sh -c`, absolute paths, implicit environment variables, or undeclared network access by default.
- It does not bind to a single LLM, Agent platform, or hosted service.
- The first version does not promise native Windows Capture; Windows is supported through WSL or a later adapter.

## 3. Users and End-to-end Flow

### 3.1 Primary Users

The first users are developers and Agent workflow authors: they want to turn a successful terminal task into a reusable Skill and use evidence to prove that it did not expand permissions or depend on invisible local state.

The second users are advanced automation users: they capture PDF, document, data-cleaning, and file-organization workflows through the CLI or local Console without needing to understand Rust or the internal IR.

### 3.2 Main CLI Flow

```bash
skilltape capture pdf-to-study
skilltape compile .skilltape/tapes/tape_01H...
skilltape lint ./pdf-to-study
skilltape verify ./pdf-to-study
skilltape export ./pdf-to-study --target generic
```

Command behavior of the full product:

| Command | Purpose | Required output |
|---|---|---|
| `init` | Create an empty Skill package | Six lintable files |
| `capture` | Record one controlled human operation | A replayable Tape directory |
| `compile` | Compile Tape into a Skill | Workflow, permissions, documentation, and fixture drafts |
| `lint` | Review the Schema and policy | Stable diagnostics, JSON output, and an explicit exit code |
| `replay` | Execute in a temporary workspace | Run logs and intermediate results |
| `verify` | Replay and run assertions | Receipt and pass/fail status |
| `export` | Generate a platform package | A target directory ready to submit to GitHub |
| `console` | Start the local viewer | Timeline, Diff, permission, and Receipt pages |

## 4. Core Domain Model

### 4.1 Tape

Tape uses a versioned JSONL event stream; each event has a stable sequence number, relative time, source, and sensitive-data handling state. The minimum event set is:

- `session.started` / `session.finished`
- `terminal.command`: program, arguments, working directory, exit code, and stdout/stderr references
- `filesystem.created` / `modified` / `moved` / `deleted`
- `permission.requested` / `permission.decided`
- `environment.snapshot`: only explicitly allowlisted fields
- `capture.warning`: missing events, truncated output, or redaction notices

Tape does not store raw secrets by default. Command arguments and output are redacted by rules before they are written to JSONL; raw content is stored temporarily only in a local diagnostic directory explicitly selected by the user, and never enters an export package.

### 4.2 Workflow IR

Workflow is the sole executable intermediate representation and uses the current versioned `workflow.yaml` schema. The first-version action set is fixed to `exec`, `script`, `file`, and `assert`.

Every step must have a stable `id`, input references, a timeout, output declarations, and a source event range. The Compiler only generates structured steps; the Runner does not interpret Markdown or directly execute model-generated natural language.

### 4.3 SkillPackage

SkillPackage continues to use the current six core files:

```text
skilltape.yaml
workflow.yaml
permissions.json
skilltape.lock
SKILL.md
README.md
```

The full product additionally adds optional directories:

```text
fixtures/
  input/
  expected/
  assertions.yaml
receipts/
tapes/
```

The core package must remain Git-friendly; every generated field must be traceable through a source path or Tape event id.

### 4.4 Run and Receipt

Run is a controlled execution for a specific input and runs in a temporary workspace by default. A Receipt records at least the Skill package hash, Tape/Workflow versions, environment summary, per-step status, command exit codes, input and output hashes, permission decisions, assertion results, duration, and cancellation or failure reason.

Receipt must not write secret plaintext; logs use references, hashes, and truncated summaries.

## 5. System Architecture

### 5.1 Rust Workspace

The existing `skilltape-schema`, `skilltape-core`, and `skilltape-cli` continue as the foundation. The full product adds the following responsibilities:

```text
crates/
  skilltape-schema/       Versioned models and JSON Schema
  skilltape-core/         SkillPackage, diagnostics, templates, and public contracts
  skilltape-tape/         Tape event model, JSONL storage, and redaction
  skilltape-capture/      PTY, command capture, file watching, and permission prompts
  skilltape-compiler/     Deterministic compilation from Tape → Workflow/Skill
  skilltape-policy/       Permission rules, risk levels, and policy explanations
  skilltape-runner/       Temporary workspaces, process supervision, and output capture
  skilltape-verify/       Fixtures, assertions, Receipt, and replay diffs
  skilltape-export/       Generic packages and Agent platform adapters
  skilltape-cli/          Command parsing, output, configuration, and process orchestration
```

Each crate must have independent unit and integration tests; cross-crate behavior is tested through stable interfaces rather than coupling through shared internal state.

### 5.2 Optional Local Console

Console is launched by the local CLI and reads Tape, Skill, and Receipt from the same workspace. The first version uses a local HTTP service and React/TypeScript pages; Console is not a runtime dependency of the CLI/SDK.

The Console’s read-only page order is:

1. Capture timeline: events, commands, file changes, and redaction markers.
2. Compile review: Workflow steps, variables, source events, and Diff.
3. Permission review: processes, files, network, secret access, and risk levels.
4. Verify run: step status, log summaries, assertions, and Receipt.

## 6. Data Flow and Security Policy

```text
PTY/File watcher
        ↓
  Redacted Tape JSONL
        ↓
  Deterministic Compiler + optional local model proposal
        ↓
  Versioned Workflow + permissions + fixtures
        ↓
  Schema validation + Policy Engine
        ↓
  Temporary Workspace Runner
        ↓
  Assertions + Receipt + export
```

Security rules:

- Capture only watches the workspace and terminal session explicitly selected by the user.
- All paths are normalized to workspace-relative paths; absolute paths, `..`, and Windows drive/UNC paths are rejected.
- The `exec` program must appear in `permissions.json`, and input references in its arguments must already be declared.
- The network is disabled by default; the host, method, and request origin must be explicitly declared.
- Environment variables are not read by default; secrets may be passed only through references confirmed by the local user and are not written to Tape/Receipt.
- The Runner uses temporary directories, resource limits, timeouts, and a cancellable token; unbounded background processes are prohibited.
- Every policy rejection produces a stable diagnostic and an explainable reason; there is no silent downgrade.

## 7. Full Implementation Phases

### Phase 0: Foundation (completed)

Delivers the current `init`, `lint`, schema, package loader, diagnostics, examples, JSON output, and baseline CI gates.

Completion criteria: the foundation workspace builds independently, the minimal package lints, and tests and Clippy pass.

### Phase 1：Capture

Delivers terminal PTY capture, file watching, event JSONL, real-time redaction, session recovery, and Capture permission prompts.

Completion criteria: a real terminal workflow can be captured on macOS/Linux, Tape can be read for replay, and event loss and sensitive fields are clearly reported.

### Phase 2：Compiler

Delivers deterministic Tape analysis, variable extraction, Workflow generation, permission inference, an SKILL.md draft, and a human-review manifest.

Model integration is only a pluggable suggester: model output first enters a proposal file and must pass schema, policy, and user confirmation before it can be written to the formal Skill.

Completion criteria: the same Tape produces a stable Workflow in no-model mode; with a model, only naming, variables, and explanations improve without changing the security boundary.

### Phase 3: Replay, Verify, Receipt

Delivers a temporary-workspace Runner, command/file output capture, timeout cancellation, fixtures, assertions, replay diffs, and Receipt.

Completion criteria: a compiled Skill can be replayed repeatedly in a clean temporary directory; undeclared permissions are always rejected; Receipts for the same input are comparable.

### Phase 4: Export and Adapters

Deliver generic Agent Skill export first, then add platform adapters according to real user volume. Adapters are responsible only for file layout, metadata, and invocation conventions; they do not duplicate core execution logic.

Completion criteria: an export directory can be submitted directly to GitHub and passes this repository’s lint/verify; a failure in any adapter does not affect the generic format.

### Phase 5: Local Console

Delivers read-only timeline, Workflow Diff, permission Diff, Verify run, and Receipt pages; Console reads artifacts through a versioned local API and owns no second business logic.

Completion criteria: users can review a Capture, confirm permissions, view failed steps, and download a Receipt without opening a terminal.

### Phase 6: Release and Ecosystem

Delivers installation scripts, cross-platform releases, a documentation site, contribution templates, fixture/adapter plugin interfaces, GitHub Actions templates, and a security disclosure process.

Completion criteria: a new user completes Capture → Compile → Verify within 5 minutes; third parties can contribute adapters or assertions through public interfaces only.

## 8. Testing and Quality Gates

Every phase must include all of the following:

- Unit tests: pure functions, schemas, redaction, policy, and state transitions.
- Integration tests: Tape/Workflow/Receipt contracts across crates.
- CLI tests: success, rejection, cancellation, JSON output, and stable exit codes.
- Fixture replay: stable inputs, expected file hashes, and expected diagnostics.
- Security tests: path escapes, command injection, secret leakage, undeclared permissions, network bypass, and background processes.
- Cross-platform tests: macOS/Linux, with PTY and file watching isolated behind adapters.

Merge gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, CLI smoke tests, fixture verification, and secret scanning all pass.

## 9. Full Product Acceptance Criteria

The full product is complete only when the entire chain below passes:

1. Users can capture a real terminal/file workflow locally.
2. Tape can be redacted, stored, recovered, and reviewed.
3. The Compiler can generate a deterministic Workflow, permission manifest, Skill documentation, and fixtures.
4. Schema/Policy can prevent path, process, network, and secret privilege escalation.
5. The Runner can replay in a temporary workspace and support cancellation, timeouts, and failure recovery.
6. Verify can generate a comparable Receipt that identifies specific failed steps and differences.
7. Export can generate a generic Skill package and at least one tested platform adapter.
8. Console can read local artifacts to support timeline, permission, Diff, and Receipt review.
9. The main chain works without configuring a cloud service; the model is an optional enhancement rather than a hard dependency.
10. A new user can follow the README to create a first Skill that passes verification within 5 minutes.

## 10. Key Decisions

- The CLI/SDK is the product core; Console must not become a hidden dependency of core capabilities.
- Complete one secure, replayable vertical flow first, then expand platforms and UI.
- JSON Schema, JSONL, YAML, Markdown, and Receipt are all public, reviewable, versioned file contracts.
- All model capabilities must remain in a suggestion layer after the deterministic Compiler/Policy.
- The full product’s first commercial/communications loop is “real operation → verifiable Skill → GitHub-shareable,” not a cloud marketplace.
