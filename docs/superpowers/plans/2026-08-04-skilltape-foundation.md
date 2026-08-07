# SkillTape Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first independently testable SkillTape vertical slice: a Rust workspace that can create, load, and lint a portable Skill package using the versioned `skilltape.yaml`, `workflow.yaml`, `permissions.json`, and `skilltape.lock` contracts.

**Architecture:** The foundation uses a Cargo workspace with a schema crate containing typed domain models and JSON Schema validation, a core crate owning package I/O and cross-file validation, and a CLI crate exposing `init` and `lint`. The implementation does not include capture, LLM providers, replay, or Web UI yet; later plans will consume the stable package and diagnostic interfaces created here.

**Tech Stack:** Rust stable, Cargo workspace, Rust 2021 edition, synchronous foundation code, `serde`, `serde_json`, `serde_yaml`, `jsonschema`, `clap`, `thiserror`, `assert_cmd`, `predicates`, and `tempfile`.

## Global Constraints

- The repository is the independent project at `/Users/chumanic/skilltape`; do not modify `/Users/chumanic/genkoy`.
- The product is local-first and must not require a cloud service for package creation or linting.
- `workflow.yaml` is the executable intermediate representation; `SKILL.md` is documentation and cannot be the sole source of execution behavior.
- LLM output is not part of this foundation and must never bypass Schema or policy validation.
- The MVP action set is limited to `exec`, `script`, `file`, and `assert`.
- All package paths are workspace-relative; absolute paths, path traversal, implicit environment variables, and arbitrary `sh -c` commands are invalid.
- Unknown schema fields are rejected unless they use the explicit `x-` extension prefix.
- Rust toolchain setup is required before code work because `rustc` is not installed in the current environment; use `brew install rust`, then verify `rustc --version` and `cargo --version`.
- Every task ends with focused verification and a conventional commit on the local `main` branch.
- No GitHub remote or push is required for this plan; remote configuration waits for an explicit repository URL.

## Scope Decomposition

The complete design spans independent subsystems. This plan intentionally covers only the package contract and CLI foundation so it produces a working, reviewable deliverable. Follow-up work is consolidated in these tracked records:

- [Full product implementation plan](2026-08-05-skilltape-full-product.md) covers Capture, Compiler, Policy, Runner, Verify, Receipt, Export, and Local Console.
- [Console release implementation plan](2026-08-07-skilltape-console-release.md) covers packaged Console assets, installation, discovery, and release verification.

## File Map

The foundation creates these boundaries:

~~~text
skilltape/
├── Cargo.toml
├── rust-toolchain.toml
├── schemas/
│   ├── skill/v1.json
│   ├── workflow/v1.json
│   ├── permissions/v1.json
│   └── lock/v1.json
├── crates/
│   ├── skilltape-schema/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── skilltape-core/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── skilltape-cli/
│       ├── Cargo.toml
│       └── src/main.rs
├── examples/minimal-skill/
│   ├── skilltape.yaml
│   ├── workflow.yaml
│   ├── permissions.json
│   ├── skilltape.lock
│   ├── SKILL.md
│   ├── README.md
│   └── fixtures/
└── tests/fixtures/invalid-skill/
~~~

`skilltape-schema` owns Rust types and schema IDs. `skilltape-core` owns package loading, cross-file validation, template generation, and diagnostics. `skilltape-cli` owns argument parsing, terminal formatting, and exit-code mapping only.

---

### Task 0: Install and pin the Rust toolchain

**Files:**
- Create: `rust-toolchain.toml`

**Interfaces:**
- Produces a repository-local stable toolchain declaration used by every later Cargo command.

- [ ] **Step 1: Install Rust using the approved system package manager**

Run:

~~~bash
brew install rust
~~~

Expected: Homebrew installs `rustc` and `cargo` without changing the SkillTape repository.

- [ ] **Step 2: Verify the toolchain is available**

Run:

~~~bash
rustc --version
cargo --version
~~~

Expected: both commands print versions and exit with code 0.

- [ ] **Step 3: Add the repository toolchain file**

Create `rust-toolchain.toml`:

~~~toml
[toolchain]
channel = "stable"
profile = "minimal"
components = ["rustfmt", "clippy"]
~~~

- [ ] **Step 4: Verify formatting and lint components**

Run:

~~~bash
rustfmt --version
cargo clippy --version
~~~

Expected: both commands exit with code 0.

- [ ] **Step 5: Commit the toolchain declaration**

~~~bash
git add rust-toolchain.toml
git commit -m "chore: pin stable rust toolchain"
~~~

### Task 1: Create the Cargo workspace and package skeletons

**Files:**
- Create: `Cargo.toml`
- Create: `crates/skilltape-schema/Cargo.toml`
- Create: `crates/skilltape-schema/src/lib.rs`
- Create: `crates/skilltape-core/Cargo.toml`
- Create: `crates/skilltape-core/src/lib.rs`
- Create: `crates/skilltape-cli/Cargo.toml`
- Create: `crates/skilltape-cli/src/main.rs`

**Interfaces:**
- Produces workspace packages named `skilltape-schema`, `skilltape-core`, and `skilltape-cli`.
- `skilltape-schema` exports `SchemaId`.
- `skilltape-core` depends on `skilltape-schema` and exports `SkillPackage`.
- `skilltape-cli` depends on `skilltape-core` and produces the `skilltape` binary.

- [ ] **Step 1: Write the workspace manifest**

Create `Cargo.toml`:

~~~toml
[workspace]
members = [
  "crates/skilltape-schema",
  "crates/skilltape-core",
  "crates/skilltape-cli",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
assert_cmd = "2"
clap = { version = "4", features = ["derive"] }
jsonschema = "0.26"
predicates = "3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
tempfile = "3"
thiserror = "2"
~~~

- [ ] **Step 2: Define the schema crate manifest and minimal public API**

Create `crates/skilltape-schema/Cargo.toml`:

~~~toml
[package]
name = "skilltape-schema"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
~~~

Create `crates/skilltape-schema/src/lib.rs`:

~~~rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaId {
    SkillV1,
    WorkflowV1,
    PermissionsV1,
    LockV1,
}

impl SchemaId {
    pub const fn uri(self) -> &'static str {
        match self {
            Self::SkillV1 => "skilltape.dev/skill/v1",
            Self::WorkflowV1 => "skilltape.dev/workflow/v1",
            Self::PermissionsV1 => "skilltape.dev/permissions/v1",
            Self::LockV1 => "skilltape.dev/lock/v1",
        }
    }
}
~~~

- [ ] **Step 3: Define the core and CLI manifests**

`crates/skilltape-core/Cargo.toml` must depend on `jsonschema`, `serde`, `serde_json`, `serde_yaml`, `thiserror`, and the local `skilltape-schema` crate.

`crates/skilltape-cli/Cargo.toml` must depend on `clap`, `skilltape-core`, `serde_json`, and `thiserror`.

Both packages must declare `edition.workspace = true` and `license.workspace = true`.

- [ ] **Step 4: Add a compiling CLI**

Create `crates/skilltape-cli/src/main.rs`:

~~~rust
fn main() {
    println!("SkillTape foundation");
}
~~~

- [ ] **Step 5: Run the workspace baseline**

~~~bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
~~~

Expected: all commands exit 0.

- [ ] **Step 6: Commit the workspace skeleton**

~~~bash
git add Cargo.toml crates
git commit -m "chore: create skilltape cargo workspace"
~~~

### Task 2: Define typed Skill package models

**Files:**
- Modify: `crates/skilltape-schema/src/lib.rs`
- Create: `crates/skilltape-schema/src/model.rs`
- Create: `crates/skilltape-schema/tests/model_roundtrip.rs`

**Interfaces:**
- Produces `SkillManifest`, `EntryPoint`, `InputSpec`, `OutputSpec`, `Workflow`, `Step`, `Permissions`, and `LockFile`.
- Produces `Step::Exec`, `Step::Script`, `Step::File`, and `Step::Assert` using serde’s `action` discriminator.
- Produces `has_expected_schema()` on `SkillManifest`, `Workflow`, `Permissions`, and `LockFile`.

- [ ] **Step 1: Write the failing model round-trip test**

Create `crates/skilltape-schema/tests/model_roundtrip.rs`:

~~~rust
use skilltape_schema::model::{Step, Workflow};

#[test]
fn parses_exec_step_from_yaml() {
    let yaml = r#"
schema: skilltape.dev/workflow/v1
steps:
  - id: extract-text
    action: exec
    program: pdftotext
    args:
      - "{{ inputs.source_pdf }}"
      - "work/input.txt"
    timeout_ms: 60000
"#;

    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow should parse");
    assert_eq!(workflow.steps.len(), 1);
    match &workflow.steps[0] {
        Step::Exec(step) => {
            assert_eq!(step.id, "extract-text");
            assert_eq!(step.program, "pdftotext");
            assert_eq!(step.args.len(), 2);
        }
        other => panic!("expected exec step, got {other:?}"),
    }
}
~~~

Run:

~~~bash
cargo test -p skilltape-schema --test model_roundtrip
~~~

Expected: FAIL because `model` and `Workflow` do not exist.

- [ ] **Step 2: Implement the manifest and workflow models**

Define these public types in `model.rs`:

~~~rust
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Workflow {
    pub schema: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Step {
    Exec(ExecStep),
    Script(ScriptStep),
    File(FileStep),
    Assert(AssertStep),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExecStep {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub outputs: Vec<StepOutput>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StepOutput {
    pub path: String,
    #[serde(rename = "type")]
    pub output_type: String,
}
~~~

Also define these exact fields:

- `SkillManifest { schema, name, version, description, engine, entrypoint, inputs, outputs, targets }`.
- `EntryPoint { workflow, permissions, lockfile }`.
- `InputSpec { id, input_type, required, description }`.
- `OutputSpec { id, output_type, path }`.
- `StepOutput { path, output_type }`.
- `ScriptStep { id, path, args, timeout_ms, outputs }`.
- `FileStep { id, operation, from, to }`.
- `AssertStep { id, assertion }` where `assertion` is `AssertionSpec { assertion_type, path, schema, hash }`.
- `Permissions { schema, filesystem, process, network, secrets }`.
- `FilesystemPermissions { read, write }`.
- `ProcessPermissions { executables, max_processes, default_timeout_ms }`.
- `NetworkPermissions { enabled, allow_hosts }`.
- `SecretPermissions { read_environment }`.
- `LockFile { schema, engine, tools, scripts }`.

Use serde field renames for the YAML keys `type`, `from`, and `to` where Rust names would be ambiguous.

- [ ] **Step 3: Add schema URI validation methods**

Implement:

~~~rust
impl Workflow {
    pub fn has_expected_schema(&self) -> bool {
        self.schema == "skilltape.dev/workflow/v1"
    }
}
~~~

Add equivalent methods to `SkillManifest`, `Permissions`, and `LockFile`, then re-export `model` from `lib.rs`.

- [ ] **Step 4: Run the focused tests**

~~~bash
cargo test -p skilltape-schema --test model_roundtrip
cargo test -p skilltape-schema
~~~

Expected: PASS.

- [ ] **Step 5: Commit the typed contract**

~~~bash
git add crates/skilltape-schema
git commit -m "feat: define skill package models"
~~~

### Task 3: Add versioned JSON Schemas and document validation

**Files:**
- Create: `schemas/skill/v1.json`
- Create: `schemas/workflow/v1.json`
- Create: `schemas/permissions/v1.json`
- Create: `schemas/lock/v1.json`
- Modify: `crates/skilltape-schema/src/lib.rs`
- Create: `crates/skilltape-schema/src/validation.rs`
- Create: `crates/skilltape-schema/tests/schema_validation.rs`

**Interfaces:**
- Produces `validate_json(schema_id: SchemaId, document: &serde_json::Value) -> Result<(), Vec<SchemaDiagnostic>>`.
- Produces `SchemaDiagnostic { instance_path: String, keyword: String, message: String }`.
- Loads schema text with `include_str!` so the CLI does not depend on the current working directory.

- [ ] **Step 1: Write failing schema tests**

Cover these cases in `schema_validation.rs`:

~~~rust
#[test]
fn rejects_workflow_with_unknown_action() {
    let document = serde_json::json!({
        "schema": "skilltape.dev/workflow/v1",
        "steps": [{
            "id": "unsafe",
            "action": "shell",
            "program": "sh",
            "args": [],
            "timeout_ms": 1000
        }]
    });
    let errors = validate_json(SchemaId::WorkflowV1, &document).expect_err("must reject");
    assert!(errors.iter().any(|error| error.keyword == "enum"));
}

#[test]
fn rejects_absolute_output_path() {
    let document = serde_json::json!({
        "schema": "skilltape.dev/workflow/v1",
        "steps": [{
            "id": "write",
            "action": "file",
            "operation": "copy",
            "from": "input.txt",
            "to": "/tmp/output.txt"
        }]
    });
    let errors = validate_json(SchemaId::WorkflowV1, &document).expect_err("must reject");
    assert!(errors.iter().any(|error| error.keyword == "pattern"));
}

#[test]
fn accepts_minimal_skill_manifest() {
    let document = serde_json::json!({
        "schema": "skilltape.dev/skill/v1",
        "name": "minimal-skill",
        "version": "0.1.0",
        "description": "A minimal SkillTape package.",
        "engine": {"min_version": "0.1.0"},
        "entrypoint": {
            "workflow": "workflow.yaml",
            "permissions": "permissions.json",
            "lockfile": "skilltape.lock"
        },
        "inputs": [],
        "outputs": [],
        "targets": ["generic-agent-skill"]
    });
    validate_json(SchemaId::SkillV1, &document).expect("manifest should validate");
}
~~~

Run:

~~~bash
cargo test -p skilltape-schema --test schema_validation
~~~

Expected: FAIL because the schemas and validator do not exist.

- [ ] **Step 2: Write the four JSON Schema documents**

Each schema must require its exact `schema` URI. The workflow schema must restrict `action` to `exec`, `script`, `file`, and `assert`; the permissions schema must require `filesystem`, `process`, `network`, and `secrets`; the lock schema must require `engine`, `tools`, and `scripts`.

The workflow schema must reject absolute paths and `..` path segments. Each schema must reject unknown fields with `additionalProperties: false`, while allowing extension fields matching `^x-` through a dedicated `patternProperties` entry.

- [ ] **Step 3: Implement the validator**

Use `jsonschema::validator_for` and collect every validation error:

~~~rust
pub fn validate_json(
    schema_id: SchemaId,
    document: &serde_json::Value,
) -> Result<(), Vec<SchemaDiagnostic>> {
    let schema = schema_value(schema_id);
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| vec![SchemaDiagnostic::from_schema_error(error)])?;
    let errors = validator
        .iter_errors(document)
        .map(SchemaDiagnostic::from_validation_error)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
~~~

- [ ] **Step 4: Run schema tests and formatting**

~~~bash
cargo fmt --all
cargo test -p skilltape-schema --test schema_validation
cargo clippy -p skilltape-schema --all-targets -- -D warnings
~~~

Expected: PASS with no Clippy warnings.

- [ ] **Step 5: Commit the schemas and validator**

~~~bash
git add schemas crates/skilltape-schema
git commit -m "feat: validate versioned skill schemas"
~~~

### Task 4: Implement package loading and cross-file validation

**Files:**
- Modify: `crates/skilltape-core/src/lib.rs`
- Create: `crates/skilltape-core/src/package.rs`
- Create: `crates/skilltape-core/src/diagnostic.rs`
- Create: `crates/skilltape-core/tests/package_validation.rs`

**Interfaces:**
- Produces `SkillPackage::load(root: impl AsRef<Path>) -> Result<LoadedSkillPackage, PackageError>`.
- Produces `LoadedSkillPackage { root, manifest, workflow, permissions, lockfile }`.
- Produces `LintReport { errors, warnings, files_checked }`.
- Produces `Diagnostic { code, level, file, path, message }`.
- Produces `LoadedSkillPackage::lint(&self, strict: bool) -> LintReport`.

- [ ] **Step 1: Write failing package tests**

Create a temporary valid package with these files:

~~~text
skilltape.yaml
workflow.yaml
permissions.json
skilltape.lock
SKILL.md
README.md
~~~

Test these cases:

~~~rust
#[test]
fn loads_all_required_package_files() { }

#[test]
fn reports_missing_entrypoint_file() { }

#[test]
fn reports_workflow_program_without_process_permission() { }

#[test]
fn reports_step_output_outside_declared_write_scope() { }

#[test]
fn strict_mode_turns_environment_mismatch_into_error() { }
~~~

Run:

~~~bash
cargo test -p skilltape-core --test package_validation
~~~

Expected: FAIL because package loading and linting are not implemented.

- [ ] **Step 2: Implement deterministic package loading**

Load exactly these files from the package root:

~~~rust
const REQUIRED_FILES: [&str; 6] = [
    "skilltape.yaml",
    "workflow.yaml",
    "permissions.json",
    "skilltape.lock",
    "SKILL.md",
    "README.md",
];
~~~

Parse YAML with `serde_yaml`, JSON with `serde_json`, and convert parse failures into `PackageError::InvalidFile { file, source }` without exposing local file contents in the error.

- [ ] **Step 3: Implement cross-file validation rules**

The linter must emit these stable diagnostic codes:

~~~text
PKG001 missing required file
PKG002 entrypoint path mismatch
PKG003 workflow schema mismatch
PKG004 undeclared executable
PKG005 undeclared filesystem read
PKG006 undeclared filesystem write
PKG007 absolute or traversal path
PKG008 undeclared input reference
PKG009 output not declared by manifest
PKG010 lockfile mismatch
~~~

Each diagnostic must include the source filename and YAML/JSON path where available. A policy violation is always an error, never a warning.

- [ ] **Step 4: Run focused tests and all foundation tests**

~~~bash
cargo test -p skilltape-core --test package_validation
cargo test --workspace
~~~

Expected: PASS.

- [ ] **Step 5: Commit the package loader and linter**

~~~bash
git add crates/skilltape-core
git commit -m "feat: load and lint skill packages"
~~~

### Task 5: Implement `skilltape init`

**Files:**
- Modify: `crates/skilltape-core/src/lib.rs`
- Create: `crates/skilltape-core/src/template.rs`
- Modify: `crates/skilltape-cli/src/main.rs`
- Create: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/tests/init_command.rs`

**Interfaces:**
- Produces `create_skill_template(root: &Path, name: &str) -> Result<(), TemplateError>`.
- Produces CLI subcommand `skilltape init <name> --output <path>`.
- `init` returns exit code 0 on creation and exit code 1 when the target exists without `--force`.

- [ ] **Step 1: Write failing CLI tests**

Use `assert_cmd` and `tempfile`:

~~~rust
#[test]
fn init_creates_a_lintable_skill_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("minimal-skill");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "minimal-skill", "--output"])
        .arg(&output)
        .assert()
        .success();

    assert!(output.join("skilltape.yaml").exists());
    assert!(output.join("workflow.yaml").exists());
    assert!(output.join("permissions.json").exists());
    assert!(output.join("skilltape.lock").exists());
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("existing");
    std::fs::create_dir_all(&output).expect("directory");
    std::fs::write(output.join("README.md"), "keep me").expect("file");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "existing", "--output"])
        .arg(&output)
        .assert()
        .failure();
}
~~~

Run:

~~~bash
cargo test -p skilltape-cli --test init_command
~~~

Expected: FAIL because the subcommand is not implemented.

- [ ] **Step 2: Implement the template generator**

Generate the six required files with the requested name substituted only in metadata and Markdown. The generated `workflow.yaml` must contain a valid empty `steps` list, and `permissions.json` must default to empty read/write/executable/host lists with network disabled.

Reject names that are empty, contain `/`, contain `\\\\`, or contain whitespace at either end. Keep the target path explicit; do not delete or recursively overwrite an existing directory.

- [ ] **Step 3: Implement the Clap command model**

Use this command shape:

~~~rust
#[derive(clap::Subcommand)]
enum Command {
    Init {
        name: String,
        #[arg(long)]
        output: std::path::PathBuf,
        #[arg(long)]
        force: bool,
    },
    Lint {
        path: std::path::PathBuf,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
    },
}
~~~

Keep formatting and exit-code mapping in the CLI crate; call core functions for all file and validation behavior.

- [ ] **Step 4: Run the command tests and manual smoke test**

~~~bash
cargo test -p skilltape-cli --test init_command
target/debug/skilltape init smoke-skill --output /tmp/skilltape-smoke-skill
target/debug/skilltape lint /tmp/skilltape-smoke-skill
~~~

Expected: the package is created and lint exits 0.

- [ ] **Step 5: Commit the init command**

~~~bash
git add crates/skilltape-core crates/skilltape-cli
git commit -m "feat: add skilltape init command"
~~~

### Task 6: Implement `skilltape lint`

**Files:**
- Modify: `crates/skilltape-cli/src/main.rs`
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/output.rs`
- Create: `crates/skilltape-cli/tests/lint_command.rs`

**Interfaces:**
- Produces `skilltape lint <path>` with human-readable output by default.
- Produces stable JSON output with `--json`:

~~~json
{
  "files_checked": 6,
  "errors": [],
  "warnings": []
}
~~~

- Maps `LintReport.errors` to exit code 2 for schema/package failures and exit code 3 for policy violations.

- [ ] **Step 1: Write failing CLI lint tests**

Add tests for:

~~~rust
#[test]
fn lint_accepts_the_checked_in_minimal_skill() { }

#[test]
fn lint_prints_stable_policy_code_for_undeclared_executable() { }

#[test]
fn lint_json_output_contains_files_checked_and_errors() { }
~~~

The tests must assert exit code, diagnostic code, and the presence of the source file; they must not assert terminal color or spacing.

Run:

~~~bash
cargo test -p skilltape-cli --test lint_command
~~~

Expected: FAIL because `lint` is not wired to the CLI.

- [ ] **Step 2: Implement human-readable output**

Format each diagnostic as:

~~~text
error[PKG004] workflow.yaml:steps[0].program
  executable "python" is not declared in permissions.json
~~~

Print a final summary:

~~~text
Checked 6 files: 0 errors, 0 warnings
~~~

- [ ] **Step 3: Implement JSON output and exit-code mapping**

Serialize `LintReport` with `serde_json` and write only JSON to stdout when `--json` is used. Send progress and fatal CLI errors to stderr so CI can parse stdout.

- [ ] **Step 4: Run all foundation verification**

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p skilltape-cli -- lint /tmp/skilltape-smoke-skill
cargo run -p skilltape-cli -- lint /tmp/skilltape-smoke-skill --json
~~~

Expected: all commands exit 0 and the JSON command emits valid JSON on stdout.

- [ ] **Step 5: Commit the lint command**

~~~bash
git add crates/skilltape-cli
git commit -m "feat: add skilltape lint command"
~~~

### Task 7: Add the checked-in example, fixtures, and documentation

**Files:**
- Create: `examples/minimal-skill/skilltape.yaml`
- Create: `examples/minimal-skill/workflow.yaml`
- Create: `examples/minimal-skill/permissions.json`
- Create: `examples/minimal-skill/skilltape.lock`
- Create: `examples/minimal-skill/SKILL.md`
- Create: `examples/minimal-skill/README.md`
- Create: `examples/minimal-skill/fixtures/input/sample.txt`
- Create: `tests/fixtures/invalid-skill/skilltape.yaml`
- Create: `tests/fixtures/invalid-skill/workflow.yaml`
- Create: `tests/fixtures/invalid-skill/permissions.json`
- Create: `tests/fixtures/invalid-skill/skilltape.lock`
- Create: `tests/fixtures/invalid-skill/SKILL.md`
- Create: `tests/fixtures/invalid-skill/README.md`
- Modify: `README.md`

**Interfaces:**
- Produces one valid package used by every CLI smoke test.
- Produces one invalid fixture that reliably emits `PKG004` and `PKG007`.
- Documents installation, `init`, `lint`, current MVP scope, and the fact that Capture/Compiler/Verify are separate follow-up plans.

- [ ] **Step 1: Add a valid minimal package**

The example must use only the foundation contract and contain one `exec` step with executable `printf`, a workspace-relative output file, a matching process permission, and network disabled. It is linted only; it is not executed by this plan.

- [ ] **Step 2: Add invalid fixtures**

The invalid workflow must contain:

~~~yaml
schema: skilltape.dev/workflow/v1
steps:
  - id: unsafe
    action: exec
    program: python
    args:
      - "../../outside.txt"
    timeout_ms: 1000
~~~

The fixture must fail for both an undeclared executable and a traversal path.

- [ ] **Step 3: Update the root README**

Add a Quick Start section with these exact commands:

~~~bash
cargo run -p skilltape-cli -- init my-skill --output ./my-skill
cargo run -p skilltape-cli -- lint ./my-skill
~~~

State that Rust stable is required, that no provider is needed for `init`/`lint`, and that the current plan does not yet implement Capture, Compiler, Verify, or Console.

- [ ] **Step 4: Run documentation and fixture verification**

~~~bash
cargo run -p skilltape-cli -- lint examples/minimal-skill
cargo run -p skilltape-cli -- lint examples/minimal-skill --json
cargo run -p skilltape-cli -- lint tests/fixtures/invalid-skill
~~~

Expected: the valid package exits 0, and the invalid fixture exits 3 with `PKG004` and `PKG007`.

- [ ] **Step 5: Commit the example and documentation**

~~~bash
git add README.md examples tests/fixtures
git commit -m "docs: add foundation examples and quickstart"
~~~

### Task 8: Final foundation gate and handoff

**Files:**
- Modify: `README.md` only if the Quick Start command is inaccurate.
- Modify: `docs/design/2026-08-04-skilltape-design.md` only if a verified contract difference is discovered.

**Interfaces:**
- Produces a clean local `main` branch with a working `skilltape init` and `skilltape lint` vertical slice.
- Produces the exact interfaces required by the Capture and Compiler plans: `SkillPackage::load`, `LintReport`, `Diagnostic`, typed `Step`, and versioned schema IDs.

- [ ] **Step 1: Run the clean verification matrix**

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p skilltape-cli -- init final-smoke --output /tmp/skilltape-final-smoke
cargo run -p skilltape-cli -- lint /tmp/skilltape-final-smoke
cargo run -p skilltape-cli -- lint examples/minimal-skill --json
~~~

Expected: every command exits 0; the invalid fixture test remains the only intentional non-zero CLI case and is asserted in the test suite rather than the final command list.

- [ ] **Step 2: Confirm the repository state**

~~~bash
git status --short --branch
git log --oneline -8
~~~

Expected: the branch is `main`, there are no uncommitted changes, and every task commit is visible.

- [ ] **Step 3: Record the foundation result**

Update the root README with the verified commands and the [documentation index](../../README.md).

- [ ] **Step 4: Commit the final handoff note**

~~~bash
git add README.md
git commit -m "docs: record foundation verification"
~~~

## Verification Matrix

The foundation is complete only when these commands pass in a clean checkout:

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p skilltape-cli -- lint examples/minimal-skill
cargo run -p skilltape-cli -- lint examples/minimal-skill --json
~~~

The following are explicitly out of this plan and must not be faked as complete:

- PTY or filesystem Capture
- LLM Provider calls
- Workflow compilation from Tape
- guarded-local or container Replay Runner
- Receipt generation
- React Console
- GitHub remote setup or push
