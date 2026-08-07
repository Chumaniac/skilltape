# SkillTape Full Product Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Archival note: This pre-migration plan preserves its historical setup, remote guidance, dates, task evidence, commands, and intent. Current implementation and release facts are tracked in [docs/release-readiness.md](../../release-readiness.md).

**Goal:** Expand the current Foundation MVP into a local-first, capturable, compilable, reviewable, replayable, verifiable, and exportable full SkillTape product flow.

**Architecture:** The Rust workspace owns the deterministic core for Tape, Capture, Compiler, Policy, Runner, Verify, Receipt, and Export; the CLI is the primary product entry point. The optional Local Console reads the same Tape, Skill, and Receipt through a versioned local HTTP API, without duplicating business logic or becoming a runtime dependency of the CLI/SDK.

**Tech Stack:** Rust stable, Tokio, serde/serde_json/serde_yaml, JSON Schema, portable-pty, notify, Axum, tracing, React, TypeScript, Vite, SSE, and GitHub Actions.

## Global Constraints

- The product core must support local operation without a cloud service, account, or fixed model provider.
- `workflow.yaml` is the sole executable intermediate representation; `SKILL.md` cannot drive execution on its own.
- The first-version action set is fixed to `exec`, `script`, `file`, and `assert`.
- All package paths must be workspace-relative; absolute paths, `..`, and Windows drive/UNC paths are rejected.
- Network, environment-variable, secret, process, and file access are denied by default and must be explicitly declared through `permissions.json`.
- Arbitrary `sh -c`, unbounded background processes, and implicit environment-variable expansion are prohibited by default.
- The model produces proposals only; a proposal must pass schema, policy, and user confirmation before entering the formal Skill package.
- Tape, Workflow, Skill, Run, and Receipt must all carry a version or stable schema ID.
- All sensitive content is redacted before being written to Tape, logs, Receipt, or an export package; tests must not commit real secrets.
- The first phase supports macOS/Linux; Windows uses WSL or a later adapter and must not block the main flow.
- Every task must write a failing test before the minimal implementation; every task ends with scoped verification and a Conventional Commit.
- Do not configure a GitHub remote, push, or create a PR unless the user provides an explicit repository URL, owner, and target branch.
- The current Foundation includes `skilltape-schema`, `skilltape-core`, `skilltape-cli`, `init`, `lint`, examples, and baseline tests; later tasks must not break the existing six-file package contract.

---

## 0. Target Repository Structure and Interface Boundaries

Final full product structure:

```text
crates/
  skilltape-schema/      Versioned schemas, models, and JSON Schema validation
  skilltape-core/        SkillPackage, diagnostics, templates, and public contracts
  skilltape-tape/        Tape event model, JSONL storage, and redaction
  skilltape-capture/     PTY, file watching, and Capture sessions
  skilltape-compiler/    Deterministic compilation from Tape → Workflow/Skill
  skilltape-policy/      Permission inference, risk levels, and policy decisions
  skilltape-runner/      Temporary workspaces, process supervision, and resource limits
  skilltape-verify/      Fixtures, assertions, Receipt, and diffs
  skilltape-export/      Generic packages and platform adapters
  skilltape-cli/         CLI, configuration, command orchestration, and exit codes
apps/
  skilltape-console/     Optional React/Vite local viewer
schemas/
  tape/v1.json
  run/v1.json
  receipt/v1.json
```

Common command exit codes are fixed as follows: `0` success, `2` package/schema/input error, `3` policy or verify failure, `4` capture/runtime failure, and `5` user cancellation.

Final local workspace structure:

```text
.skilltape/
  config.yaml
  tapes/tape_demo/events.jsonl
  tapes/tape_demo/manifest.json
  runs/run_demo/run.json
  receipts/run_demo.json
  cache/
```

---

### Task 1: Establish the Versioned Tape Contract

**Files:**
- Create: `crates/skilltape-tape/Cargo.toml`
- Create: `crates/skilltape-tape/src/lib.rs`
- Create: `crates/skilltape-tape/src/event.rs`
- Create: `crates/skilltape-tape/src/session.rs`
- Create: `schemas/tape/v1.json`
- Create: `crates/skilltape-tape/tests/tape_roundtrip.rs`
- Modify: `Cargo.toml`

**Interfaces:**

```rust
pub const TAPE_SCHEMA_V1: &str = "skilltape.dev/tape/v1";

pub struct TapeEvent {
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub kind: TapeEventKind,
    pub source: EventSource,
    pub payload: serde_json::Value,
    pub redaction: RedactionState,
}

pub enum TapeEventKind {
    SessionStarted,
    SessionFinished,
    TerminalCommand,
    FilesystemChanged,
    PermissionRequested,
    PermissionDecided,
    EnvironmentSnapshot,
    CaptureWarning,
}

pub struct TapeManifest {
    pub schema: String,
    pub id: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub platform: String,
    pub workspace_root: String,
    pub event_count: u64,
}
```

- [ ] Step 1: Write JSON Schema and round-trip tests for each `TapeEventKind`.
- [ ] Step 2: Write tests covering sequence monotonicity, rejection of unknown kinds, schema ID validation, and single-line JSONL serialization.
- [ ] Step 3: Implement strongly typed events and `serde` conversion, rejecting negative times, empty IDs, and absolute workspace paths.
- [ ] Step 4: Run `cargo test -p skilltape-tape --test tape_roundtrip` and `cargo clippy -p skilltape-tape --all-targets -- -D warnings`.
- [ ] Step 5: Commit `feat: define versioned tape events`.

### Task 2: Implement Tape JSONL Persistence and Recovery

**Files:**
- Create: `crates/skilltape-tape/src/store.rs`
- Create: `crates/skilltape-tape/src/ids.rs`
- Create: `crates/skilltape-tape/tests/tape_store.rs`

**Interfaces:**

```rust
pub struct TapeStore { root: PathBuf }

impl TapeStore {
    pub fn create(root: impl Into<PathBuf>, manifest: TapeManifest) -> Result<Self, TapeStoreError>;
    pub fn append(&self, event: &TapeEvent) -> Result<(), TapeStoreError>;
    pub fn finish(&self, finished_at_ms: u64) -> Result<TapeManifest, TapeStoreError>;
    pub fn read_manifest(&self) -> Result<TapeManifest, TapeStoreError>;
    pub fn read_events(&self) -> Result<impl Iterator<Item = Result<TapeEvent, TapeStoreError>>, TapeStoreError>;
}
```

- [ ] Step 1: Write tests covering creation, append, recovery, finishing, duplicate sequence values, and truncated JSONL.
- [ ] Step 2: Implement atomic manifest writes, fsync-backed appends, line-by-line recovery, and preservation of existing events on failure.
- [ ] Step 3: Use a sortable local generator for IDs; tests must not depend on real time or random output.
- [ ] Step 4: Run all Tape crate tests and check temporary-directory cleanup.
- [ ] Step 5: Commit `feat: persist tape sessions as jsonl`.

### Task 3: Implement Capture Redaction and the Environment Allowlist

**Files:**
- Create: `crates/skilltape-capture/Cargo.toml`
- Create: `crates/skilltape-capture/src/redaction.rs`
- Create: `crates/skilltape-capture/src/environment.rs`
- Create: `crates/skilltape-capture/tests/redaction.rs`

**Interfaces:**

```rust
pub struct RedactionConfig {
    pub secret_names: BTreeSet<String>,
    pub patterns: Vec<Regex>,
    pub max_output_bytes: usize,
}

pub fn redact_text(input: &str, config: &RedactionConfig) -> RedactedText;
pub fn snapshot_environment(allowlist: &[String]) -> EnvironmentSnapshot;
```

- [ ] Step 1: Write tests covering API keys, Bearer tokens, password arguments, environment-variable values, long-output truncation, and UTF-8 boundaries.
- [ ] Step 2: Implement redaction before writing to Tape; preserve field names, lengths, and hashes without preserving secret plaintext.
- [ ] Step 3: Implement a read-only allowlisted environment snapshot that returns an empty set by default.
- [ ] Step 4: Run secret scanning with fixed fixtures and ensure test output contains no raw secrets.
- [ ] Step 5: Commit `feat: redact captured secrets`.

### Task 4: Implement PTY Terminal Capture

**Files:**
- Modify: `crates/skilltape-capture/Cargo.toml`
- Create: `crates/skilltape-capture/src/pty.rs`
- Create: `crates/skilltape-capture/src/session.rs`
- Create: `crates/skilltape-capture/tests/pty_capture.rs`

**Interfaces:**

```rust
pub struct CaptureOptions {
    pub command: String,
    pub args: Vec<String>,
    pub workspace: PathBuf,
    pub env_allowlist: Vec<String>,
    pub output_limit: usize,
}

pub async fn capture_terminal(
    options: CaptureOptions,
    store: TapeStore,
    cancel: CancellationToken,
) -> Result<CaptureSummary, CaptureError>;
```

- [ ] Step 1: Write fake PTY adapter tests without directly depending on an interactive terminal.
- [ ] Step 2: Integrate `portable-pty` and record command, args, cwd, stdout/stderr, exit code, and terminal dimensions.
- [ ] Step 3: Handle Ctrl-C, terminal exit, output truncation, timeouts, and child-process reaping.
- [ ] Step 4: Run a `printf`/temporary-script Capture integration test in macOS/Linux CI.
- [ ] Step 5: Commit `feat: capture terminal sessions into tape`.

### Task 5: Implement Filesystem-Change Capture

**Files:**
- Create: `crates/skilltape-capture/src/filesystem.rs`
- Create: `crates/skilltape-capture/tests/filesystem_capture.rs`

**Interfaces:**

```rust
pub enum FilesystemChangeKind { Created, Modified, Moved, Deleted }

pub struct FilesystemChange {
    pub kind: FilesystemChangeKind,
    pub path: String,
    pub previous_path: Option<String>,
    pub content_hash: Option<String>,
    pub size: Option<u64>,
}

pub async fn watch_workspace(
    root: &Path,
    tx: mpsc::Sender<FilesystemChange>,
    cancel: CancellationToken,
) -> Result<(), FilesystemCaptureError>;
```

- [ ] Step 1: Write temporary-workspace tests covering create/modify/move/delete and event deduplication.
- [ ] Step 2: Integrate `notify` and normalize OS paths to workspace-relative paths.
- [ ] Step 3: Reject paths outside the root and record hash/size instead of saving full file contents by default.
- [ ] Step 4: Merge filesystem events with PTY events by time window and test stable ordering.
- [ ] Step 5: Commit `feat: capture workspace file changes`.

### Task 6: Add the `skilltape capture` CLI

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/capture_command.rs`
- Create: `crates/skilltape-cli/tests/capture_command.rs`

**Interfaces:**

```text
skilltape capture <name> [--workspace <path>] [--command <program>] [--output <tape-path>] [--json]
```

- [ ] Step 1: Write CLI tests checking the default workspace, output Tape path, cancellation, and error exit codes.
- [ ] Step 2: Orchestrate PTY/file watcher/redaction/store and output a manifest summary when Capture ends.
- [ ] Step 3: Add `--allow-env`, `--max-output-bytes`, and an explicit confirmation prompt.
- [ ] Step 4: Run Capture integration tests and use the generated Tape as the next-phase fixture.
- [ ] Step 5: Commit `feat: add capture command`.

### Task 7: Establish the Compiler Domain Model and Provenance

**Files:**
- Create: `crates/skilltape-compiler/Cargo.toml`
- Create: `crates/skilltape-compiler/src/lib.rs`
- Create: `crates/skilltape-compiler/src/provenance.rs`
- Create: `crates/skilltape-compiler/tests/provenance.rs`

**Interfaces:**

```rust
pub struct CompileRequest {
    pub tape: TapeSession,
    pub name: String,
    pub target: CompileTarget,
}

pub struct CompileOutput {
    pub workflow: Workflow,
    pub permissions: Permissions,
    pub skill_markdown: String,
    pub fixtures: FixtureDraft,
    pub provenance: Vec<StepProvenance>,
}

pub struct StepProvenance {
    pub step_id: String,
    pub event_sequences: Vec<u64>,
    pub source_summary: String,
}

pub trait Compiler {
    fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompileError>;
}
```

- [ ] Step 1: Write tests ensuring every generated step is associated with at least one Tape event and missing sources fail directly.
- [ ] Step 2: Define the deterministic compile target and stable sorting rules.
- [ ] Step 3: Serialize provenance into extension fields in `skilltape.yaml` or a separate `compile.json`.
- [ ] Step 4: Run the same Tape twice and compare complete output hashes.
- [ ] Step 5: Commit `feat: define compiler provenance contracts`.

### Task 8: Implement the Model-free Deterministic Compiler

**Files:**
- Create: `crates/skilltape-compiler/src/deterministic.rs`
- Create: `crates/skilltape-compiler/src/steps.rs`
- Create: `crates/skilltape-compiler/tests/deterministic_compile.rs`

- [ ] Step 1: Write failing tests for exec/file/fixture Tape with explicit Workflow, permission, and output expectations.
- [ ] Step 2: Merge adjacent terminal/file events into `exec`, `file`, and `assert` steps.
- [ ] Step 3: Infer the minimum permission set from actual commands and file changes, with the network disabled by default.
- [ ] Step 4: Generate `SKILL.md`, `workflow.yaml`, `permissions.json`, fixture drafts, and provenance.
- [ ] Step 5: Validate all outputs through the existing `SkillPackage::load().lint(false)`.
- [ ] Step 6: Commit `feat: compile deterministic skills from tape`.

### Task 9: Add the Optional Model Proposal Interface

**Files:**
- Create: `crates/skilltape-compiler/src/proposal.rs`
- Create: `crates/skilltape-compiler/src/provider.rs`
- Create: `crates/skilltape-compiler/tests/proposal.rs`

**Interfaces:**

```rust
pub trait ProposalProvider {
    async fn propose(&self, input: ProposalInput) -> Result<WorkflowProposal, ProviderError>;
}

pub struct WorkflowProposal {
    pub workflow_patch: serde_json::Value,
    pub descriptions: BTreeMap<String, String>,
    pub model: String,
    pub input_hash: String,
}

pub fn apply_proposal(
    base: CompileOutput,
    proposal: WorkflowProposal,
    policy: &PolicyEngine,
) -> Result<CompileOutput, ProposalError>;
```

- [ ] Step 1: Write fake-provider tests verifying that the model cannot add undeclared programs, paths, or network access.
- [ ] Step 2: Persist the proposal file with the input hash, model metadata, and human-confirmation state.
- [ ] Step 3: Apply the proposal patch to the deterministic base and re-run schema/lint/policy validation.
- [ ] Step 4: Provide explainable errors for provider timeouts, invalid JSON, quota errors, and offline mode.
- [ ] Step 5: Commit `feat: add optional compiler proposals`.

### Task 10: Add the `skilltape compile` CLI

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/compile_command.rs`
- Create: `crates/skilltape-cli/tests/compile_command.rs`

**Interfaces:**

```text
skilltape compile <tape-path> --output <skill-path> [--provider <name>] [--accept-proposal]
```

- [ ] Step 1: Write CLI tests for deterministic compile success, an existing output, a Tape without provenance, and an offline provider.
- [ ] Step 2: Orchestrate the Compiler without calling a provider by default; atomically move the output after writing it to a temporary directory.
- [ ] Step 3: Call the provider only when `--provider` is explicit; return code 3 when a proposal is unconfirmed.
- [ ] Step 4: Run `lint` automatically after compilation; a lint failure must not produce a “publishable” status.
- [ ] Step 5: Commit `feat: add compile command`.

### Task 11: Extract the Policy Engine

**Files:**
- Create: `crates/skilltape-policy/Cargo.toml`
- Create: `crates/skilltape-policy/src/lib.rs`
- Create: `crates/skilltape-policy/src/rules.rs`
- Create: `crates/skilltape-policy/src/risk.rs`
- Create: `crates/skilltape-policy/tests/policy_rules.rs`

**Interfaces:**

```rust
pub struct PolicyEngine { rules: PolicyRules }

pub struct PolicyDecision {
    pub allowed: bool,
    pub code: String,
    pub reason: String,
    pub risk: RiskLevel,
}

impl PolicyEngine {
    pub fn check_command(&self, program: &str, args: &[String], permissions: &Permissions) -> PolicyDecision;
    pub fn check_path(&self, path: &str, access: FileAccess, permissions: &Permissions) -> PolicyDecision;
    pub fn check_network(&self, host: &str, permissions: &Permissions) -> PolicyDecision;
    pub fn check_environment(&self, name: &str, permissions: &Permissions) -> PolicyDecision;
}
```

- [ ] Step 1: Write positive and negative tests for every policy code, covering paths, processes, networks, environments, and secrets.
- [ ] Step 2: Unify the existing `PKG001`–`PKG010` diagnostics with runtime decision codes.
- [ ] Step 3: Implement the `low`/`medium`/`high`/`critical` risk levels and user-readable explanations.
- [ ] Step 4: Ensure Compiler, Lint, and Runner share the same PolicyEngine rules.
- [ ] Step 5: Commit `feat: centralize skilltape policy engine`.

### Task 12: Implement the Controlled Replay Runner

**Files:**
- Create: `crates/skilltape-runner/Cargo.toml`
- Create: `crates/skilltape-runner/src/lib.rs`
- Create: `crates/skilltape-runner/src/workspace.rs`
- Create: `crates/skilltape-runner/src/process.rs`
- Create: `crates/skilltape-runner/tests/runner.rs`

**Interfaces:**

```rust
pub struct RunRequest {
    pub package: SkillPackage,
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    pub limits: ResourceLimits,
}

pub struct RunEvent {
    pub sequence: u64,
    pub step_id: String,
    pub status: StepStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

pub async fn run_skill(
    request: RunRequest,
    policy: PolicyEngine,
    events: mpsc::Sender<RunEvent>,
    cancel: CancellationToken,
) -> Result<RunSummary, RunError>;
```

- [ ] Step 1: Write fake process adapter tests covering success, non-zero, timeout, cancellation, and spawn failure.
- [ ] Step 2: Create a temporary workspace, copy allowed fixture inputs, and prohibit access to the original user directory.
- [ ] Step 3: Call PolicyEngine before and after each step; do not start the process on rejection and record the decision.
- [ ] Step 4: Implement stdout/stderr truncation, resource limits, child-process-tree reaping, and stable RunEvent values.
- [ ] Step 5: Run real `printf` and failure-script integration tests on macOS/Linux.
- [ ] Step 6: Commit `feat: add guarded replay runner`.

### Task 13: Implement Fixtures, Assertions, and Receipt

**Files:**
- Create: `crates/skilltape-verify/Cargo.toml`
- Create: `crates/skilltape-verify/src/lib.rs`
- Create: `crates/skilltape-verify/src/assertions.rs`
- Create: `crates/skilltape-verify/src/receipt.rs`
- Create: `schemas/run/v1.json`
- Create: `schemas/receipt/v1.json`
- Create: `crates/skilltape-verify/tests/verify.rs`

**Interfaces:**

```rust
pub enum Assertion {
    FileExists { path: String },
    FileHash { path: String, sha256: String },
    FileTextContains { path: String, text: String },
    CommandExit { step_id: String, code: i32 },
}

pub struct Receipt {
    pub schema: String,
    pub run_id: String,
    pub skill_hash: String,
    pub status: ReceiptStatus,
    pub steps: Vec<ReceiptStep>,
    pub assertions: Vec<AssertionResult>,
    pub policy_decisions: Vec<PolicyDecisionSummary>,
}

pub async fn verify_run(request: VerifyRequest) -> Result<Receipt, VerifyError>;
```

- [ ] Step 1: Write tests for assertion success, assertion failure, missing files, hash mismatch, and Receipt redaction.
- [ ] Step 2: Implement fixture input copying, Runner invocation, assertion execution, and step differences.
- [ ] Step 3: Generate a schema-versioned Receipt and save all outputs as hashes/truncated summaries.
- [ ] Step 4: Ensure the same package/input produces a comparable Receipt in a stable environment.
- [ ] Step 5: Commit `feat: verify runs and generate receipts`.

### Task 14: Add the `replay` and `verify` CLIs

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/run_command.rs`
- Create: `crates/skilltape-cli/tests/verify_command.rs`

**Interfaces:**

```text
skilltape replay <skill-path> [--input <path>] [--json]
skilltape verify <skill-path> [--input <path>] [--receipt <path>] [--json]
```

- [ ] Step 1: Write tests for clean-package success, policy rejection, assertion failure, timeout, and JSON output.
- [ ] Step 2: `replay` outputs only a Run summary; `verify` additionally writes a Receipt and returns 3 for verification failure.
- [ ] Step 3: Write all fatal errors to stderr; `--json` stdout contains only one schema-versioned JSON document.
- [ ] Step 4: Connect init → capture fixture → compile → verify into a CLI journey test.
- [ ] Step 5: Commit `feat: add replay and verify commands`.

### Task 15: Implement the Generic Exporter

**Files:**
- Create: `crates/skilltape-export/Cargo.toml`
- Create: `crates/skilltape-export/src/lib.rs`
- Create: `crates/skilltape-export/src/generic.rs`
- Create: `crates/skilltape-export/tests/generic_export.rs`

**Interfaces:**

```rust
pub trait Exporter {
    fn target_id(&self) -> &'static str;
    fn export(&self, package: &SkillPackage, output: &Path) -> Result<ExportManifest, ExportError>;
}

pub struct ExportManifest {
    pub target: String,
    pub files: Vec<String>,
    pub package_hash: String,
}
```

- [ ] Step 1: Write tests for the export file list, path safety, overwrite protection, and package hash.
- [ ] Step 2: Implement the `generic` target, copying the six-file package, fixtures, Receipt references, and license/readme metadata.
- [ ] Step 3: Force lint before export; a failure must not produce a “complete” manifest.
- [ ] Step 4: Commit `feat: add generic skill exporter`.

### Task 16: Implement the First Platform Adapter

**Files:**
- Modify: `crates/skilltape-export/src/lib.rs`
- Create: `crates/skilltape-export/src/claude_code.rs`
- Create: `crates/skilltape-export/tests/claude_code_export.rs`

**Interfaces:**

```text
skilltape export <skill-path> --target claude-code --output <directory>
```

- [ ] Step 1: Fix the platform output layout as `.claude/skills/<skill-name>/SKILL.md` with adjacent Workflow/fixture files and verify the file list with a fixture.
- [ ] Step 2: The platform adapter converts only metadata and file layout; it does not change Workflow or permissions.
- [ ] Step 3: Test repeated export, target-directory safety, missing platform fields, and generic target fallback.
- [ ] Step 4: Commit `feat: export skills for claude code`.

### Task 17: Add the `export` CLI and Adapter Registry

**Files:**
- Create: `crates/skilltape-export/src/registry.rs`
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/tests/export_command.rs`

- [ ] Step 1: Write tests for `--target generic`, `--target claude-code`, an unknown target, and an output-directory conflict.
- [ ] Step 2: Implement the registry, target list, and JSON manifest output.
- [ ] Step 3: Standardize CLI code 2 for input errors and code 3 for lint/export policy failures.
- [ ] Step 4: Commit `feat: add export command and registry`.

### Task 18: Establish the Local Console API

**Files:**
- Create: `apps/skilltape-console-api/Cargo.toml`
- Create: `apps/skilltape-console-api/src/main.rs`
- Create: `apps/skilltape-console-api/src/routes.rs`
- Create: `apps/skilltape-console-api/src/read_model.rs`
- Create: `apps/skilltape-console-api/tests/routes.rs`

**Interfaces:**

```text
GET /api/v1/workspaces
GET /api/v1/workspaces/:id/tapes
GET /api/v1/tapes/:id/events
GET /api/v1/skills/:id/diff
GET /api/v1/runs/:id
GET /api/v1/receipts/:id
GET /api/v1/runs/:id/events  (SSE)
```

- [ ] Step 1: Write fixture-workspace route tests that prohibit access outside the root.
- [ ] Step 2: Implement a read-only read model; the API directly reuses core/tape/verify types.
- [ ] Step 3: Implement sequence numbers, reconnects, and end events for SSE run events.
- [ ] Step 4: Bind the API to localhost by default and print a security warning when external binding is explicitly allowed.
- [ ] Step 5: Commit `feat: add local console read api`.

### Task 19: Implement Console Timeline and Review Pages

**Files:**
- Create: `apps/skilltape-console/package.json`
- Create: `apps/skilltape-console/index.html`
- Create: `apps/skilltape-console/vite.config.ts`
- Create: `apps/skilltape-console/tsconfig.json`
- Create: `apps/skilltape-console/src/main.tsx`
- Create: `apps/skilltape-console/src/styles.css`
- Create: `apps/skilltape-console/src/api.ts`
- Create: `apps/skilltape-console/src/pages/Timeline.tsx`
- Create: `apps/skilltape-console/src/pages/CompileReview.tsx`
- Create: `apps/skilltape-console/src/pages/PermissionReview.tsx`
- Create: `apps/skilltape-console/src/pages/ReceiptView.tsx`
- Create: `apps/skilltape-console/tests/console.spec.ts`

- [ ] Step 1: Use API fixtures to write tests for page routes, empty states, loading, errors, and long logs.
- [ ] Step 2: Implement the timeline, source event links, Workflow Diff, and permission Diff.
- [ ] Step 3: Implement Receipt step status, assertion results, policy decisions, and JSON download.
- [ ] Step 4: Keep pages read-only; do not execute commands or parse natural language into Workflow in the browser.
- [ ] Step 5: Commit `feat: add local skilltape console`.

### Task 20: Integrate `skilltape console`

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/console_command.rs`
- Create: `crates/skilltape-cli/tests/console_command.rs`

**Interfaces:**

```text
skilltape console [--workspace <path>] [--port <port>] [--open]
```

- [ ] Step 1: Write tests for localhost binding, port conflicts, workspace validation, and `--open`.
- [ ] Step 2: Start the API and static UI, and output the access address and security notice.
- [ ] Step 3: Reap child processes when Console closes without modifying workspace artifacts.
- [ ] Step 4: Commit `feat: add console command`.

### Task 21: Installation, Configuration, and GitHub Actions

**Files:**
- Modify: `README.md`
- Create: `docs/guides/installation.md`
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/skill-verify.yml`
- Create: `scripts/install.sh`
- Create: `scripts/install.ps1`

- [ ] Step 1: Write complete command examples in the README from installation through Capture → Compile → Verify.
- [ ] Step 2: CI runs fmt, Clippy, workspace tests, example lint, and the invalid-fixture failure assertion.
- [ ] Step 3: The Skill workflow template runs only the local CLI and does not upload Tape, Receipt, or secrets.
- [ ] Step 4: Installation scripts validate download checksums, support pinned versions, and do not overwrite an existing binary on failure.
- [ ] Step 5: Commit `docs: add installation and github workflows`.

### Task 22: Plugin and Adapter Development Interface

**Files:**
- Create: `docs/reference/tape-format.md`
- Create: `docs/reference/adapter-api.md`
- Create: `crates/skilltape-export/src/plugin.rs`
- Create: `crates/skilltape-export/tests/plugin_contract.rs`

**Plugin protocol:**

```text
skilltape-export-plugin --input ./export-request.json --output ./exported
```

The plugin reads one `ExportRequest` JSON document from the input file, writes
one `ExportManifest` JSON document to stdout, writes diagnostics to stderr,
returns `0` on success, `2` for invalid input, and `3` for policy/export
failure. The host rejects any manifest path outside the requested output root
and re-runs `skilltape lint` on the produced package.

- [ ] Step 1: Document the fixed `Exporter`, schema version, manifest, and capability-negotiation contracts.
- [ ] Step 2: Define process boundaries, input directories, output manifests, and error JSON for third-party exporters.
- [ ] Step 3: Test unknown schemas, missing capabilities, output paths outside the root, and plugin-crash isolation.
- [ ] Step 4: Commit `docs: publish adapter extension contract`.

### Task 23: Security, Performance, and Cross-platform Release Gates

**Files:**
- Create: `tests/security/path_escape.rs`
- Create: `tests/security/secret_leak.rs`
- Create: `tests/integration/full_journey.rs`
- Create: `benchmarks/capture_compile.rs`
- Create: `SECURITY.md`
- Modify: `README.md`

- [ ] Step 1: Write the full journey: capture → compile → lint → verify → receipt → export.
- [ ] Step 2: Add tests for path escapes, command injection, network bypass, environment leakage, Receipt secret leakage, and background processes.
- [ ] Step 3: Add observable benchmarks for 10k Tape events, a 100-step Workflow, and 1GB logs without blocking functional tests on undefined hard thresholds.
- [ ] Step 4: Run the full matrix in macOS/Linux CI and record platform differences for PTY/file watching.
- [ ] Step 5: Publish `SECURITY.md`, the vulnerability disclosure process, a compatibility table, and the version policy.
- [ ] Step 6: Commit `test: add full product security and release gates`.

## Final Acceptance Matrix

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p skilltape-cli -- init final-smoke --output /tmp/skilltape-final-smoke
cargo run -p skilltape-cli -- capture demo --command printf --output ./.skilltape/tapes/tape_demo
cargo run -p skilltape-cli -- compile ./.skilltape/tapes/tape_demo --output ./demo-skill
cargo run -p skilltape-cli -- lint ./demo-skill
cargo run -p skilltape-cli -- verify ./demo-skill --json
cargo run -p skilltape-cli -- export ./demo-skill --target generic --output ./exported-skill
cargo run -p skilltape-cli -- console --workspace .
```

Full product completion criteria:

- Capture produces a recoverable, redacted, reviewable Tape.
- The Compiler deterministically generates a lintable Skill in no-model mode; model proposals may enhance descriptions only and cannot bypass policy.
- Replay executes in a temporary workspace and supports cancellation, timeouts, and privilege-escalation rejection.
- Verify generates a comparable Receipt that identifies specific step and assertion differences.
- Generic and at least one platform exporter pass the same lint/verify gates.
- Console read-only displays the timeline, Workflow/permission Diff, run status, and Receipt.
- The full main flow requires no cloud service, and a new user can complete a first verified Skill within 5 minutes.
