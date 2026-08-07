# Complete SkillTape Product Design

> Status: Design draft v0.1
> Date: 2026-08-04
> Project: Standalone open-source repository, independent of GenkoyAI
> Working name: SkillTape

> Archival note: This is a historical design record. Its research and proposed design decisions are dated and do not assert that every described capability shipped. Current implementation and release status are tracked in [docs/release-readiness.md](../release-readiness.md).

## 1. Summary

SkillTape is a local-first, replay-verifiable Agent Skill compiler.

It captures a real terminal and filesystem workflow completed by a user and compiles it into an auditable, reusable Agent Skill package that can be committed to GitHub. The Skill package includes:

- `SKILL.md` for Agents
- `workflow.yaml` for runtime execution
- `permissions.json` for security review
- fixtures, assertions, and a Receipt for verification

Core product promise:

> One real operation produces an evidence-backed, replayable, shareable Agent Skill.

SkillTape does not treat the LLM as a direct executor. The LLM may only help interpret the trace and generate structured Workflow IR; all actual command, file, and network access must pass through the schema, permission policy, and Replay.

## 2. Research Basis and Product Judgments

This design references the GitHub Trending daily, weekly, and monthly lists as of 2026-08-04:

- [GitHub daily trending list](https://github.com/trending?since=daily)
- [GitHub weekly trending list](https://github.com/trending?since=weekly)
- [GitHub monthly trending list](https://github.com/trending?since=monthly)

The visible trends at the time suggested concentrated growth in Agent Skills, model gateways, Agent memory, browser/desktop operations, local inference, and multimodal tools. The resulting product choices were:

1. Use developer tools and Agent infrastructure as the primary entry point for early Stars.
2. Use visualized traces, Diffs, and Receipts to create a demonstration experience that ordinary users can understand.
3. Use local execution, open formats, and multi-platform adapters to avoid vendor lock-in.
4. Make “verifiable” the core value that distinguishes SkillTape from ordinary Prompt/Skill generators.

These are design judgments intended to increase the probability of early adoption, not guarantees of any specific number of Stars.

## 3. Product Positioning and Target Users

### 3.1 Primary Users

The first user group is developers and AI workflow authors:

- Write Skills for Claude Code, Codex, Cursor, Cline, or other Agents
- Avoid hand-writing complex workflow instructions
- Need to verify that a Skill actually works
- Want to publish personal expertise as an open-source project that others can Fork

The second user group is advanced non-developers:

- Batch-organize files
- Process PDFs and documents
- Generate reports
- Clean data
- Reuse office workflows

Ordinary users do not need to understand the internal IR; they can use the local Web UI to Capture, review, and run workflows.

### 3.2 Core Problems

Current AI Skill/Agent workflows usually have four problems:

1. A Skill is mainly a document and cannot prove that it is executable in practice.
2. Generated commands may expand file, network, or process permissions.
3. Workflows depend on the author's local environment and are difficult to reproduce.
4. Other people have difficulty judging whether a Skill is trustworthy through GitHub Review.

SkillTape addresses this by breaking one task into:

```text
Operation trace → structured workflow → permission policy → Replay result
```

### 3.3 Non-Goals

The proposed MVP does not include:

- A cloud Skill marketplace
- User accounts and billing
- A self-hosted large model
- Multi-Agent collaborative orchestration
- Full-featured desktop RPA
- Arbitrary GUI operations in the first release
- Compatibility with every Agent platform in the first release
- Default execution of high-risk system operations by third-party Skills

## 4. Product Contract and Core Objects

SkillTape keeps only four core objects:

| Object | Meaning |
|---|---|
| Tape | Structured record of one real operation |
| Skill | Reusable workflow package compiled from a Tape |
| Run | One actual execution of a Skill |
| Receipt | Evidence, logs, and result summary for a Run |

### 4.1 Tape

A Tape is not simply a video. It is an event record with sequence numbers that includes commands, output, the working directory, filesystem changes, and permission decisions.

### 4.2 Skill

A Skill is a directory that can be committed to Git. It contains documentation, Workflow IR, a permission manifest, fixtures, and tests.

### 4.3 Run

A Run is an execution instance of a Skill against a set of inputs. A Run uses a temporary workspace by default and does not modify the user's original directory directly.

### 4.4 Receipt

A Receipt records the status and duration of every step, input and output hashes, permission decisions, assertion results, and failure reasons.

## 5. User Flow

```text
Capture
  Capture a successful human workflow

Compile
  Compile the trace into Workflow IR and Skill documentation

Review
  Inspect steps, variables, file access, and permission Diff

Verify
  Replay in a temporary environment using fixtures

Share
  Export a GitHub project ready to commit and Fork
```

Ideal command flow:

```bash
skilltape capture pdf-to-study
skilltape compile tape_01H...
skilltape lint ./pdf-to-study
skilltape verify ./pdf-to-study
skilltape export ./pdf-to-study
```

## 6. MVP Boundaries and Success Criteria

### 6.1 Proposed MVP Contents

- Terminal command Capture
- Capture of file creation, modification, movement, and deletion
- Immediate event redaction
- Local models and an OpenAI-compatible provider
- Compilation from Tape to Workflow IR
- `SKILL.md` generation
- `workflow.yaml` generation
- `permissions.json` generation
- Schema and policy checks
- Fixture-driven Replay verification
- Receipt generation
- A local Web viewer
- Generic Skill package export

### 6.2 Platform Boundaries

- The first phase supports macOS and Linux
- Windows is supported later through WSL or an independent adapter
- The first phase captures only terminal activity and filesystem changes in a specified workspace
- Browser Capture belongs to the second phase
- The local Web UI is hosted by a local service first rather than using Tauri/Electron

### 6.3 Success Criteria

The historical target was for a user to go from installation to generating a verified Skill within five minutes.

The MVP acceptance criteria in this design were:

1. A user can Capture a real workflow.
2. The compiler generates a readable, verifiable Skill package.
3. Undeclared commands, paths, and network access are blocked.
4. A Skill can be replayed repeatedly with fixtures.
5. Replay generates a Receipt.
6. The generated directory can be committed directly to GitHub.
7. The complete flow works without configuring a cloud service.

## 7. Overall Architecture

### 7.1 Technology Choices

The design recommends a Rust core with a local TypeScript/React Console:

- Rust handles single-binary distribution, PTY, filesystem watching, process supervision, and runtime policy.
- TypeScript/React handles the timeline, Diffs, permission review, and Receipt display.
- Axum provides the local HTTP API and SSE.
- JSON Schema is the contract source shared by Rust and TypeScript.
- JSONL, YAML, and Markdown are used as reviewable file formats.

Suggested technology components:

| Capability | Choice |
|---|---|
| Async runtime | Tokio |
| PTY | portable-pty |
| Filesystem watching | notify |
| HTTP | Axum |
| Serialization | serde / serde_json / serde_yaml |
| Schema | JSON Schema + schemars |
| Logging | tracing |
| Web UI | React + Vite + TypeScript |
| CI | GitHub Actions |

### 7.2 Component Diagram

```text
skilltape CLI
    │
    ├── Capture Engine
    ├── Tape Store
    ├── Compiler
    ├── Policy Engine
    ├── Replay Runner
    ├── Receipt Store
    └── Local Web Server
             │
             └── React Console
```

The CLI and Web UI must reuse the same Core; the frontend must not reimplement compilation, policy, or execution logic.

## 8. Capture Engine

### 8.1 Terminal Capture

`skilltape capture <name>` starts a controlled PTY Shell. The user completes the work normally in that Shell and enters `exit` to end Capture.

Captured content:

- Command input
- stdout/stderr
- Exit code
- Current working directory
- Command duration
- Subprocess summary
- Filesystem changes

The first version does not implement system-wide hooks or attempt to record arbitrary operations that a user performs in every application.

### 8.2 Filesystem-change Capture

The user must specify a workspace. Capture Engine records changes using an initial snapshot plus filesystem watching:

- Creation
- Modification
- Movement
- Deletion
- File size
- Before-and-after hashes

File contents are not copied in full by default. Only user-specified input and output examples are placed in `fixtures/`.

### 8.3 Event Format

```json
{
  "seq": 12,
  "type": "command.finished",
  "elapsed_ms": 1842,
  "payload": {
    "exit_code": 0,
    "duration_ms": 1842
  },
  "redaction": "applied"
}
```

Event types:

- `session.started`
- `command.started`
- `command.output`
- `command.finished`
- `file.changed`
- `approval.changed`
- `session.ended`

### 8.4 Redaction

Redaction occurs before events enter the persistence layer:

- Record environment-variable names only, not their values
- Detect patterns for Tokens, Cookies, Authorization values, private keys, and common API Keys
- Replace suspected secrets in command arguments with the `<REDACTED>` marker
- Block export when redaction is uncertain
- Do not save raw events by default

## 9. Tape Storage

```text
.tapes/
└── pdf-to-study/
    └── tape_01H/
        ├── manifest.json
        ├── events.jsonl
        ├── artifacts/
        │   ├── before/
        │   └── after/
        └── redactions.json
```

Design principles:

- Append to `events.jsonl`
- Use increasing sequence numbers for events
- Preserve a `partial` Tape even after an interruption or crash
- A Tape is not a Skill and must go through compilation and review
- Do not include the raw trace in the GitHub package unless the user explicitly chooses to do so

## 10. Compile Pipeline

```text
Tape
  ↓
Trace Analyzer
  ↓
Workflow IR
  ↓
Schema Validator
  ↓
Policy Planner
  ↓
Skill Renderer
```

### 10.1 Trace Analyzer

Identifies:

- Fixed commands
- Parameterizable paths
- Input and output files
- Steps that can be merged
- Unstable data
- Required permissions

### 10.2 LLM Boundary

The LLM may only generate or modify Workflow IR. It cannot:

- Execute Shell directly
- Modify user files directly
- Approve permissions automatically
- Add an unobserved program without warning
- Generate arbitrary Shell through string concatenation

### 10.3 Compilation Failure

If schema, variable, path, or permission validation fails, generate only a diagnostic report and do not generate a runnable Skill.

## 11. Skill Package Format

```text
pdf-to-study/
├── skilltape.yaml
├── workflow.yaml
├── permissions.json
├── skilltape.lock
├── SKILL.md
├── README.md
├── fixtures/
│   ├── input/
│   └── expected/
├── scripts/
├── tests/
│   └── workflow.test.yaml
└── .gitignore
```

### 11.1 `skilltape.yaml`

```yaml
schema: skilltape.dev/skill/v1

name: pdf-to-study
version: 0.1.0
description: Convert a PDF into structured study notes.

engine:
  min_version: 0.1.0

entrypoint:
  workflow: workflow.yaml
  permissions: permissions.json
  lockfile: skilltape.lock

inputs:
  - id: source_pdf
    type: file
    required: true

outputs:
  - id: study_notes
    type: file
    path: output/study-notes.md

targets:
  - generic-agent-skill
```

### 11.2 `workflow.yaml`

```yaml
schema: skilltape.dev/workflow/v1

steps:
  - id: extract-text
    action: exec
    program: pdftotext
    args:
      - "{{ inputs.source_pdf }}"
      - "work/input.txt"
    timeout_ms: 60000
    outputs:
      - path: work/input.txt
        type: text

  - id: build-notes
    action: script
    path: scripts/build_notes.py
    args:
      - "work/input.txt"
      - "output/study-notes.md"
    timeout_ms: 120000

  - id: verify-output
    action: assert
    assertion:
      type: file_exists
      path: output/study-notes.md
```

### 11.3 Action Types

The proposed MVP supports only:

- `exec`: Execute an explicit program and its arguments
- `script`: Execute a hashed helper script contained in the package
- `file`: Copy, move, and create directories within the permitted scope
- `assert`: Check a file, JSON Schema, hash, or exit code

Arbitrary `sh -c` is not allowed as the normalized execution form.

### 11.4 Path and Variable Rules

- Inputs must be declared in advance
- Paths must be relative to the workspace
- Absolute paths are forbidden
- Implicit environment variables are forbidden
- Free-form Shell string concatenation is forbidden
- Outputs must be declared by steps
- Steps may read only declared inputs or outputs from preceding steps

### 11.5 `permissions.json`

```json
{
  "schema": "skilltape.dev/permissions/v1",
  "filesystem": {
    "read": ["inputs/**", "work/**"],
    "write": ["work/**", "output/**"]
  },
  "process": {
    "executables": ["pdftotext", "python"],
    "max_processes": 4,
    "default_timeout_ms": 120000
  },
  "network": {
    "enabled": false,
    "allow_hosts": []
  },
  "secrets": {
    "read_environment": false
  }
}
```

Undeclared capabilities are denied by default.

### 11.6 `skilltape.lock`

`skilltape.lock` is generated by `verify` to pin the verification environment and hashes for package scripts. It must not contain API Keys, Tokens, or user file contents.

```yaml
schema: skilltape.dev/lock/v1

engine:
  version: 0.1.0

tools:
  - program: pdftotext
    version: 24.02.0
    sha256: "..."
  - program: python
    version: 3.12.4

scripts:
  - path: scripts/build_notes.py
    sha256: "..."
```

When local tool versions do not match the lock file, `verify` gives a warning by default; `--strict` mode fails immediately.

### 11.7 `SKILL.md` and `README.md`

`SKILL.md` is for Agents, while `README.md` is for GitHub users. Both may be edited by hand.

SkillTape updates only the following marked block:

```markdown
<!-- skilltape:generated:start -->
Generated verification summary...
<!-- skilltape:generated:end -->
```

This prevents recompilation from overwriting documentation written by the user.

## 12. Policy Engine and Replay Runner

### 12.1 Permission Policy

Default policy:

- Deny reads and writes outside the workspace
- Disable the network by default
- Do not read secrets from environment variables
- Do not escalate privileges
- Execute only declared programs
- Require user confirmation for permission changes
- Block unobserved commands by default

### 12.2 Replay Flow

```text
fixtures/input
    ↓
Copy into a temporary workspace
    ↓
Policy Engine check
    ↓
Replay Runner execution
    ↓
File and assertion checks
    ↓
Generate Receipt
```

### 12.3 Runtime Backends

The proposed MVP provides:

1. `guarded-local`: A temporary workspace, path checks, process supervision, timeouts, and network policy.
2. `container`: Stronger isolation when Docker/Podman is available.

`guarded-local` does not claim to be a complete security sandbox. The container backend must be recommended when executing an untrusted third-party Skill.

## 13. Receipt

```text
receipts/
└── 2026-08-04T12-30-00/
    ├── receipt.json
    ├── stdout.log
    ├── stderr.log
    ├── file-diff.json
    └── summary.md
```

Receipt contains:

- Skill version
- Input file hashes
- Start and end times for every step
- Command exit codes
- Permission approval records
- Filesystem changes
- Assertion results
- Failed step and reason
- Model provider and model name
- Whether a local model was used

Success must explicitly list completed steps and assertions; failure must stop at the specific step rather than return an ambiguous success.

## 14. CLI Design

### 14.1 Commands

```text
skilltape init <name>
skilltape capture <name>
skilltape tapes list
skilltape tapes show <id>
skilltape tapes diff <id>
skilltape compile <tape>
skilltape lint <skill>
skilltape verify <skill>
skilltape run <skill>
skilltape export <skill>
skilltape ui
skilltape doctor
```

### 14.2 Common Options

```text
--json
--verbose
--no-color
--workspace <path>
--yes
--dry-run
```

### 14.3 Secure Defaults

- `run` requires permission confirmation by default
- `export` requires lint and verify to pass by default
- `--approved` is valid only for a reviewed Skill whose permissions have not changed
- `--force` does not skip policy checks; it only permits overwriting generated files

### 14.4 Exit Codes

| Exit code | Meaning |
|---:|---|
| 0 | Success |
| 1 | Argument or user-input error |
| 2 | Schema or compilation error |
| 3 | Blocked by permission policy |
| 4 | Replay verification failure |
| 5 | Local environment or model unavailable |

## 15. Local Web UI

### 15.1 Pages

#### Dashboard

Shows Tapes, Skills, recent verification results, failed tasks, and unreviewed permissions.

#### Tape Inspector

Shows the command timeline, stdout/stderr, filesystem Diffs, redaction markers, and network access.

#### Compile Review

Shows generated steps, inputs and outputs, programs, and permission Diffs. Users confirm items individually; there is no default “Allow all”.

#### Skill Review

View side by side:

- Skill Overview
- Workflow
- Permissions
- Generated Docs

#### Verify Run

Shows the real-time status of each step through SSE.

#### Receipt Viewer

Shows the complete execution Receipt as a report and supports copying a Markdown summary.

### 15.2 Local API

```text
GET  /api/tapes
GET  /api/tapes/:id/events
POST /api/tapes/:id/compile
POST /api/skills/:id/lint
POST /api/skills/:id/verify
GET  /api/runs/:id
GET  /api/runs/:id/stream
POST /api/skills/:id/export
```

### 15.3 UI Security

- Listen only on `127.0.0.1`
- Generate a random access Token on every startup
- Check the request Origin
- Set a strict CSP
- Do not allow the UI to execute Shell directly
- Route every action through the Core API

### 15.4 First Demonstration Flow

```text
capture → timeline → compile → permission Diff → verify → green Receipt → GitHub package
```

This was intended as the primary flow for the first-screen README GIF and the public demonstration video.

## 16. Provider, Capture Source, and Export Adapter

### 16.1 Provider

Provider is responsible only for structured generation and capability detection:

```text
health()
complete_structured(schema, context)
capabilities()
```

Initial implementations:

- Ollama
- LM Studio
- OpenAI-compatible HTTP API

All content sent to a provider must be a redacted Tape summary. `--offline` mode prohibits all network requests.

### 16.2 Capture Source

The proposed MVP includes a built-in `shell` Capture Source. Future extensions:

- browser
- git
- editor
- desktop

Capture Source emits uniform events without changing the Compiler or Policy Engine.

### 16.3 Export Adapter

The Core always outputs a generic Skill package. Platform adapters implement the following independently:

```text
adapter id
protocol version
detect()
validate(skill)
render(skill, output_dir)
```

Adapters should run as independent processes through JSON-RPC over stdio so that a plugin crash does not affect the core runtime.

Initial targets:

- generic-agent-skill
- claude
- codex
- cursor

MCP exposure and the browser Capture Adapter are deferred to later versions.

## 17. Skill Sharing Mechanism

### 17.1 MVP

A Git repository is the first registry; no centralized marketplace is built.

```bash
skilltape export ./pdf-to-study
git add .
git commit -m "feat: add pdf to study skill"
```

SkillTape automatically generates:

- README
- Installation instructions
- Permission summary
- Verification badge summary
- Example inputs and outputs

### 17.2 Later Versions

- `skilltape pack`
- `skilltape install <git-url>`
- `skilltape verify --strict`
- GitHub Action
- Skill index page
- Signatures and provenance

Accounts, cloud execution, and a paid marketplace are not introduced in the MVP.

## 18. Security and Threat Model

| Threat | Defense | Residual risk |
|---|---|---|
| Token recorded during Capture | Streaming redaction; environment variables are not persisted | New secrets whose patterns are not recognized |
| Malicious Skill reads user files | Default workspace restriction; permission manifest | Local mode is not hard isolation |
| Network data exfiltration | Network disabled by default; network policy recorded | External processes may bypass it; use a container |
| Prompt Injection | Treat content as data; the LLM does not execute directly | The external Agent remains a risk |
| Shell injection | `program + args[]`; arbitrary `sh -c` prohibited | The invoked program itself may be dangerous |
| Browser cross-site call to the local API | 127.0.0.1, random Token, and Origin check | Risk of the user intentionally exposing the Token |
| Script tampering | Script hashes and lock file | The user may overwrite files intentionally |

Security product copy must state clearly:

> SkillTape provides default-deny and reviewable execution; for untrusted Skills, use a container runtime rather than treating guarded-local as a complete sandbox.

## 19. Testing and Verification Strategy

### 19.1 Unit Tests

- Event serialization and ordering
- Secret redaction
- Path normalization
- Command-argument validation
- Schema validation
- Permission matching
- Receipt generation
- JSON/YAML round trips

### 19.2 Golden Fixtures

Initial fixed examples:

1. `rename-images`
2. `pdf-to-markdown`
3. `csv-to-report`
4. `git-release-notes`
5. `workspace-cleanup`

Each example contains a Tape, Skill, fixtures, an expected Receipt, and failure cases.

### 19.3 Security Tests

- Path traversal
- Absolute paths
- Shell injection
- Undeclared executables
- Undeclared network requests
- Environment-variable secrets
- Timeouts and process leaks
- Malicious YAML fields
- Untrusted script imports

### 19.4 Integration and E2E

- macOS/Linux CLI integration tests
- Docker/Podman runner tests
- Web UI Playwright tests
- SSE interruption and reconnection
- Recovery after Capture exits midway
- Explicit failure when a Provider is unavailable

### 19.5 CI Gates

Pull Requests must pass:

- Rust format
- Rust lint
- TypeScript type-check
- Unit tests
- Golden fixture Replay
- Security policy tests
- UI E2E
- Documentation example commands

## 20. Repository Structure

Target repository structure:

```text
skilltape/
├── Cargo.toml
├── crates/
│   ├── skilltape-cli/
│   ├── skilltape-core/
│   ├── skilltape-capture/
│   ├── skilltape-schema/
│   ├── skilltape-compiler/
│   ├── skilltape-policy/
│   ├── skilltape-runner/
│   └── skilltape-server/
├── apps/
│   └── console/
├── schemas/
├── examples/
├── fixtures/
├── adapters/
├── docs/
│   └── design/
├── .github/
│   └── workflows/
├── README.md
├── LICENSE-MIT
└── LICENSE-APACHE
```

Shared Schema definitions are in `schemas/`; Rust and TypeScript generate types from them to prevent drift between the two sides.

## 21. Implementation Phases

### Phase 0: Protocol Foundation

- Initialize the Rust workspace
- Define JSON Schema
- Implement Skill package read/write
- Implement `init` and `lint`
- Add the first Golden Fixture

Completion criterion: An empty Skill can be created, validated, and exported.

### Phase 1: Capture

- PTY Shell
- Command events
- Filesystem watching
- Event redaction
- Tape persistence
- `capture` and `tapes` commands

Completion criterion: Capture and reopen a complete Tape.

### Phase 2: Compiler

- Trace Analyzer
- Provider interface
- Workflow IR generation
- Permission inference
- Skill documentation generation
- Compile Review data

Completion criterion: Three example Tapes compile into valid Skills.

### Phase 3: Verify

- Policy Engine
- guarded-local runner
- Fixture Replay
- Assertions
- Receipt
- `verify` and `run`

Completion criterion: Success, failure, timeout, and permission-blocked runs all produce readable Receipts.

### Phase 4: Console

- Local HTTP API
- SSE
- Dashboard
- Tape Inspector
- Compile Review
- Verify Run
- Receipt Viewer

Completion criterion: Review and verification can be completed without CLI arguments.

### Phase 5: Public Release

- GitHub Releases
- Prebuilt macOS/Linux binaries
- Homebrew formula
- Example Skill repository
- Generic Export Adapter
- GitHub Action design

## 22. Star Growth Strategy

The project cannot rely on “many features” to gain Stars; it must reduce the cost of first understanding and first success.

The initial repository must have:

1. A one-sentence product explanation above the fold.
2. A 30–60 second GIF showing Capture → Verify.
3. One command for installation.
4. Three examples that can be copied.
5. Clear screenshots of permission Diffs and Receipts.
6. A clear explanation of “No cloud required” and “No vendor lock-in”.
7. A future roadmap for GitHub Actions and badges.
8. Independently contributable adapters, examples, and fixtures.

Initial examples should prioritize workflows whose results can be shown visually:

- PDF to Markdown
- Batch rename and organize images
- Generate a report from CSV
- Generate Git release notes
- Clean a project directory

Promotion should focus on “evidence” and “reproducibility”, not exaggerating Agent intelligence.

## 23. Major Risks and Responses

### Risk 1: Becoming an Ordinary Skill Generator

Response: Make verification, permissions, Replay, and Receipt core product capabilities; they must not be removed from the MVP.

### Risk 2: Overstated Security Commitments

Response: Clearly distinguish guarded-local from container; do not market local path blocking as a complete sandbox.

### Risk 3: Excessive Cross-platform Scope

Response: Support macOS/Linux terminal and filesystem workflows first, and defer browser and Windows support.

### Risk 4: LLM Vendor Lock-in

Response: Provider exposes only a structured-generation interface, with Ollama and OpenAI-compatible APIs supported by default.

### Risk 5: Excessive Format Complexity

Response: Keep only four Action types, linear steps, and JSON Schema in the MVP; defer conditions, loops, and multi-Agent workflows to later versions.

### Risk 6: The UI Becomes a Second System

Response: The UI calls only the Core API; all compilation, policy, execution, and Receipt logic is implemented in the Rust core.

## 24. Definition of Done

The historical SkillTape v0.1.0 design is complete only when all of the following conditions are met:

- macOS/Linux can install and start the CLI
- Terminal and filesystem workflows can be Captured
- Tapes can be recovered, viewed, and redacted
- A valid Workflow IR can be generated through a Provider
- The LLM cannot bypass IR to execute commands directly
- Permissions default to deny
- At least three Golden Fixtures pass Replay
- Permission, path, injection, and secret-leak tests pass
- Every execution has a Receipt
- The local Web UI can complete review and verification
- A generated Skill can be read without depending on SkillTape
- README contains a complete 60-second demonstration and failure cases
- Documentation example commands run in a clean environment

## 25. Final Design Conclusion

The core of SkillTape is not “making AI do more things automatically”. It is:

> Compile one successful human task into an open-source asset with explicit permissions, verifiable results, and reuse by Agents.

It gains early adoption through developer infrastructure, lowers the barrier to entry with visual Replay that ordinary users can understand, and builds a contribution ecosystem through Git and open file formats.

The MVP must maintain four boundaries:

1. Terminal and filesystem first.
2. The LLM generates structured intent only and does not execute directly.
3. Permissions default to deny, and Replay must produce evidence.
4. GitHub packages first; cloud marketplaces later.
