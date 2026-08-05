# SkillTape Full Product Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前 Foundation MVP 扩展为一条本地优先、可捕获、可编译、可审查、可回放、可验证、可导出的完整 SkillTape 产品链路。

**Architecture:** Rust workspace 承担 Tape、Capture、Compiler、Policy、Runner、Verify、Receipt 和 Export 的确定性核心；CLI 是主要产品入口。可选 Local Console 通过版本化本地 HTTP API 读取同一套 Tape、Skill 和 Receipt，不复制业务逻辑，也不成为 CLI/SDK 的运行时依赖。

**Tech Stack:** Rust stable、Tokio、serde/serde_json/serde_yaml、JSON Schema、portable-pty、notify、Axum、tracing、React、TypeScript、Vite、SSE、GitHub Actions。

## Global Constraints

- 产品核心必须支持无云端、无账号、无固定模型供应商的本地运行。
- `workflow.yaml` 是唯一可执行中间表示；`SKILL.md` 不能单独驱动执行。
- 第一版动作集合固定为 `exec`、`script`、`file`、`assert`。
- 所有包路径必须是 workspace-relative；绝对路径、`..`、Windows drive/UNC 路径拒绝。
- 网络、环境变量、秘密、进程和文件访问默认拒绝，必须通过 `permissions.json` 显式声明。
- 禁止默认执行任意 `sh -c`、无界后台进程或隐式环境变量展开。
- 模型只产生 proposal；proposal 必须经过 schema、policy 和用户确认才能进入正式 Skill 包。
- Tape、Workflow、Skill、Run、Receipt 都必须带版本或稳定 schema ID。
- 所有敏感内容在写入 Tape、日志、Receipt 或导出包前脱敏；测试不得提交真实秘密。
- 第一阶段支持 macOS/Linux；Windows 使用 WSL 或后续适配器，不阻塞主链路。
- 每个任务必须先写失败测试，再写最小实现；每个任务结束都运行 scoped verification 并提交一个 Conventional Commit。
- 不配置 GitHub remote、不推送、不创建 PR，除非用户提供明确的仓库 URL、owner 和目标分支。
- 当前 Foundation 已包含 `skilltape-schema`、`skilltape-core`、`skilltape-cli`、`init`、`lint`、示例和基础测试；后续任务不得破坏现有六文件包契约。

---

## 0. 目标仓库结构与接口边界

完整产品最终结构：

```text
crates/
  skilltape-schema/       版本化 schema、模型、JSON Schema 校验
  skilltape-core/         SkillPackage、诊断、模板、公共契约
  skilltape-tape/         Tape 事件模型、JSONL 存储、脱敏
  skilltape-capture/      PTY、文件监听、Capture 会话
  skilltape-compiler/     Tape → Workflow/Skill 确定性编译
  skilltape-policy/       权限推断、风险分级、策略决策
  skilltape-runner/       临时工作区、进程监督、资源限制
  skilltape-verify/       fixture、断言、Receipt、差异
  skilltape-export/       通用包和平台适配器
  skilltape-cli/          CLI、配置、命令编排、退出码
apps/
  skilltape-console/      可选 React/Vite 本地查看器
schemas/
  tape/v1.json
  run/v1.json
  receipt/v1.json
```

公共命令退出码固定为：`0` 成功，`2` 包/schema/输入错误，`3` policy 或 verify 失败，`4` 捕获/运行时失败，`5` 用户取消。

最终本地工作区结构：

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

### Task 1: 建立 Tape 版本化契约

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

- [ ] Step 1: 为每种 `TapeEventKind` 写 JSON Schema 和 round-trip 测试。
- [ ] Step 2: 写测试覆盖 sequence 单调性、未知 kind 拒绝、schema ID 校验和 JSONL 单行序列化。
- [ ] Step 3: 实现强类型事件与 `serde` 转换，拒绝负数时间、空 ID 和绝对 workspace 路径。
- [ ] Step 4: 运行 `cargo test -p skilltape-tape --test tape_roundtrip` 与 `cargo clippy -p skilltape-tape --all-targets -- -D warnings`。
- [ ] Step 5: 提交 `feat: define versioned tape events`。

### Task 2: 实现 Tape JSONL 存储和恢复

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

- [ ] Step 1: 写测试覆盖新建、追加、恢复、完成、重复 sequence 和截断 JSONL。
- [ ] Step 2: 实现原子 manifest 写入、追加 fsync、逐行恢复和失败时不覆盖已有事件。
- [ ] Step 3: 为 ID 使用可排序的本地生成器；测试不依赖真实时间或随机输出。
- [ ] Step 4: 运行 Tape crate 全部测试并检查临时目录清理。
- [ ] Step 5: 提交 `feat: persist tape sessions as jsonl`。

### Task 3: 实现 Capture 脱敏和环境白名单

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

- [ ] Step 1: 写测试覆盖 API key、Bearer token、密码参数、环境变量值、长输出截断和 UTF-8 边界。
- [ ] Step 2: 实现写入 Tape 前的脱敏；保留字段名、长度和哈希，不保留秘密原文。
- [ ] Step 3: 实现只读白名单环境快照，默认返回空集合。
- [ ] Step 4: 使用固定 fixtures 运行秘密扫描，确保测试输出不出现原始秘密。
- [ ] Step 5: 提交 `feat: redact captured secrets`。

### Task 4: 实现 PTY 终端 Capture

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

- [ ] Step 1: 写 fake PTY adapter 测试，不直接依赖交互式终端。
- [ ] Step 2: 接入 `portable-pty`，记录 command、args、cwd、stdout/stderr、退出码和终端尺寸。
- [ ] Step 3: 处理 Ctrl-C、终端退出、输出截断、超时和子进程回收。
- [ ] Step 4: 在 macOS/Linux CI 运行一个 `printf`/临时脚本 Capture 集成测试。
- [ ] Step 5: 提交 `feat: capture terminal sessions into tape`。

### Task 5: 实现文件变化 Capture

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

- [ ] Step 1: 写临时工作区测试覆盖 create/modify/move/delete 和事件去重。
- [ ] Step 2: 接入 `notify`，将 OS 路径规范化为 workspace-relative。
- [ ] Step 3: 拒绝 root 之外路径，记录 hash/size 而不是默认保存完整文件内容。
- [ ] Step 4: 将文件事件与 PTY 事件按时间窗口合并，测试稳定排序。
- [ ] Step 5: 提交 `feat: capture workspace file changes`。

### Task 6: 增加 `skilltape capture` CLI

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/capture_command.rs`
- Create: `crates/skilltape-cli/tests/capture_command.rs`

**Interfaces:**

```text
skilltape capture <name> [--workspace <path>] [--command <program>] [--output <tape-path>] [--json]
```

- [ ] Step 1: 写 CLI 测试检查默认 workspace、输出 Tape 路径、取消和错误退出码。
- [ ] Step 2: 编排 PTY/file watcher/redaction/store，捕获结束后输出 manifest 摘要。
- [ ] Step 3: 增加 `--allow-env`、`--max-output-bytes` 和明确的确认提示。
- [ ] Step 4: 运行 capture 集成测试并用生成 Tape 作为下一阶段 fixture。
- [ ] Step 5: 提交 `feat: add capture command`。

### Task 7: 建立 Compiler 领域模型和 provenance

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

- [ ] Step 1: 写测试确保每个生成 step 至少关联一个 Tape event，缺失来源直接报错。
- [ ] Step 2: 定义 deterministic compile target 和稳定排序规则。
- [ ] Step 3: 实现 provenance 序列化到 `skilltape.yaml` 扩展字段或独立 `compile.json`。
- [ ] Step 4: 运行相同 Tape 两次并比较完整输出哈希。
- [ ] Step 5: 提交 `feat: define compiler provenance contracts`。

### Task 8: 实现无模型的确定性 Compiler

**Files:**
- Create: `crates/skilltape-compiler/src/deterministic.rs`
- Create: `crates/skilltape-compiler/src/steps.rs`
- Create: `crates/skilltape-compiler/tests/deterministic_compile.rs`

- [ ] Step 1: 写 exec/file/fixture Tape 的失败测试，明确 Workflow、权限和 output 预期。
- [ ] Step 2: 将相邻 terminal/file 事件合并为 `exec`、`file`、`assert` 步骤。
- [ ] Step 3: 从实际命令和文件变化推断最小权限集合，默认网络关闭。
- [ ] Step 4: 生成 `SKILL.md`、`workflow.yaml`、`permissions.json`、fixtures 草稿和 provenance。
- [ ] Step 5: 通过现有 `SkillPackage::load().lint(false)` 校验所有输出。
- [ ] Step 6: 提交 `feat: compile deterministic skills from tape`。

### Task 9: 增加可选模型 Proposal 接口

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

- [ ] Step 1: 写 fake provider 测试，验证模型不可新增未声明 program/path/network。
- [ ] Step 2: 实现 proposal 文件落盘、input hash、model metadata 和人工确认状态。
- [ ] Step 3: 将 proposal patch 应用在 deterministic base 上，重新 schema/lint/policy 校验。
- [ ] Step 4: 对 provider timeout、invalid JSON、quota error 和离线模式提供可解释错误。
- [ ] Step 5: 提交 `feat: add optional compiler proposals`。

### Task 10: 增加 `skilltape compile` CLI

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/compile_command.rs`
- Create: `crates/skilltape-cli/tests/compile_command.rs`

**Interfaces:**

```text
skilltape compile <tape-path> --output <skill-path> [--provider <name>] [--accept-proposal]
```

- [ ] Step 1: 写 deterministic compile 成功、输出已存在、Tape 无来源、provider 离线的 CLI 测试。
- [ ] Step 2: 编排 Compiler，默认不调用 provider；输出临时目录后原子移动。
- [ ] Step 3: provider 仅在显式 `--provider` 时调用，proposal 未确认时命令返回 code 3。
- [ ] Step 4: 编译后自动运行 `lint`，lint 失败不得生成“可发布”状态。
- [ ] Step 5: 提交 `feat: add compile command`。

### Task 11: 抽取 Policy Engine

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

- [ ] Step 1: 写每条 policy code 的正反测试，覆盖路径、进程、网络、环境和秘密。
- [ ] Step 2: 将现有 `PKG001`–`PKG010` 诊断与运行时 decision code 统一。
- [ ] Step 3: 实现风险级别 `low`/`medium`/`high`/`critical` 和用户可读解释。
- [ ] Step 4: 确保 Compiler、Lint 和 Runner 共用同一 PolicyEngine 规则。
- [ ] Step 5: 提交 `feat: centralize skilltape policy engine`。

### Task 12: 实现受控 Replay Runner

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

- [ ] Step 1: 写 fake process adapter 测试，覆盖 success、non-zero、timeout、cancel 和 spawn failure。
- [ ] Step 2: 创建临时 workspace，复制允许的 fixture 输入，禁止访问原始用户目录。
- [ ] Step 3: 对每一步执行前后调用 PolicyEngine，拒绝时不启动进程并记录 decision。
- [ ] Step 4: 实现 stdout/stderr 截断、资源上限、子进程树回收和稳定 RunEvent。
- [ ] Step 5: 在 macOS/Linux 跑真实 `printf` 和失败脚本集成测试。
- [ ] Step 6: 提交 `feat: add guarded replay runner`。

### Task 13: 实现 Fixtures、Assertions 和 Receipt

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

- [ ] Step 1: 写断言成功、断言失败、缺失文件、hash mismatch 和 receipt redaction 测试。
- [ ] Step 2: 实现 fixture input 复制、Runner 调用、断言执行和步骤差异。
- [ ] Step 3: 生成 schema-versioned Receipt，所有输出按 hash/截断摘要保存。
- [ ] Step 4: 保证同一 package/input 在稳定环境下生成可比较 Receipt。
- [ ] Step 5: 提交 `feat: verify runs and generate receipts`。

### Task 14: 增加 `replay`、`verify` CLI

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/run_command.rs`
- Create: `crates/skilltape-cli/tests/verify_command.rs`

**Interfaces:**

```text
skilltape replay <skill-path> [--input <path>] [--json]
skilltape verify <skill-path> [--input <path>] [--receipt <path>] [--json]
```

- [ ] Step 1: 写 clean package success、policy reject、assertion failure、timeout 和 JSON output 测试。
- [ ] Step 2: `replay` 只输出 Run 摘要；`verify` 额外写 Receipt 并返回 3 表示验证失败。
- [ ] Step 3: 所有 fatal errors 写 stderr；`--json` stdout 只包含一个 schema-versioned JSON 文档。
- [ ] Step 4: 将 init → capture fixture → compile → verify 串成 CLI journey test。
- [ ] Step 5: 提交 `feat: add replay and verify commands`。

### Task 15: 实现通用 Exporter

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

- [ ] Step 1: 写 export 文件清单、路径安全、覆盖保护和 package hash 测试。
- [ ] Step 2: 实现 `generic` target，复制六文件包、fixtures、Receipt 引用和 license/readme 元数据。
- [ ] Step 3: 导出前强制 lint；失败时不产生“完成” manifest。
- [ ] Step 4: 提交 `feat: add generic skill exporter`。

### Task 16: 实现第一个平台适配器

**Files:**
- Modify: `crates/skilltape-export/src/lib.rs`
- Create: `crates/skilltape-export/src/claude_code.rs`
- Create: `crates/skilltape-export/tests/claude_code_export.rs`

**Interfaces:**

```text
skilltape export <skill-path> --target claude-code --output <directory>
```

- [ ] Step 1: 将平台输出布局固定为 `.claude/skills/<skill-name>/SKILL.md` 及其相邻 Workflow/fixture 文件，并用 fixture 验证清单。
- [ ] Step 2: 平台适配器只转换元数据和文件布局，不改变 Workflow 或 permissions。
- [ ] Step 3: 测试重复 export、目标目录安全、缺少平台字段和通用 target fallback。
- [ ] Step 4: 提交 `feat: export skills for claude code`。

### Task 17: 增加 `export` CLI 与适配器注册表

**Files:**
- Create: `crates/skilltape-export/src/registry.rs`
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/tests/export_command.rs`

- [ ] Step 1: 写 `--target generic`、`--target claude-code`、未知 target 和输出目录冲突测试。
- [ ] Step 2: 实现 registry、目标列表和 JSON manifest 输出。
- [ ] Step 3: CLI 统一 code 2 输入错误、code 3 lint/export policy 失败。
- [ ] Step 4: 提交 `feat: add export command and registry`。

### Task 18: 建立 Local Console API

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

- [ ] Step 1: 写 fixture workspace route 测试，禁止访问 root 之外路径。
- [ ] Step 2: 实现只读 read model；API 直接复用 core/tape/verify 类型。
- [ ] Step 3: 实现 SSE run events 的序号、断线重连和结束事件。
- [ ] Step 4: API 默认绑定 localhost，显式允许外部绑定时打印安全警告。
- [ ] Step 5: 提交 `feat: add local console read api`。

### Task 19: 实现 Console 时间线和审查页面

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

- [ ] Step 1: 用 API fixture 写页面路由、空状态、loading、错误和长日志测试。
- [ ] Step 2: 实现 timeline、source event link、Workflow Diff 和权限 Diff。
- [ ] Step 3: 实现 Receipt 步骤状态、断言结果、policy decision 和下载 JSON。
- [ ] Step 4: 页面只读，不在浏览器端执行命令或解析自然语言为 Workflow。
- [ ] Step 5: 提交 `feat: add local skilltape console`。

### Task 20: 集成 `skilltape console`

**Files:**
- Modify: `crates/skilltape-cli/src/commands.rs`
- Create: `crates/skilltape-cli/src/console_command.rs`
- Create: `crates/skilltape-cli/tests/console_command.rs`

**Interfaces:**

```text
skilltape console [--workspace <path>] [--port <port>] [--open]
```

- [ ] Step 1: 写 localhost bind、port conflict、workspace validation 和 `--open` 测试。
- [ ] Step 2: 启动 API 与静态 UI，输出访问地址和安全提示。
- [ ] Step 3: 关闭 Console 时回收子进程，不修改 workspace 工件。
- [ ] Step 4: 提交 `feat: add console command`。

### Task 21: 安装、配置和 GitHub Actions

**Files:**
- Modify: `README.md`
- Create: `docs/guides/installation.md`
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/skill-verify.yml`
- Create: `scripts/install.sh`
- Create: `scripts/install.ps1`

- [ ] Step 1: 写 README 从安装到 Capture → Compile → Verify 的完整命令示例。
- [ ] Step 2: CI 运行 fmt、Clippy、workspace tests、example lint 和 invalid fixture 失败断言。
- [ ] Step 3: Skill workflow 模板只运行本地 CLI，不上传 Tape、Receipt 或秘密。
- [ ] Step 4: 安装脚本校验下载 checksum、支持版本固定、失败时不覆盖现有 binary。
- [ ] Step 5: 提交 `docs: add installation and github workflows`。

### Task 22: 插件和适配器开发接口

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

- [ ] Step 1: 固定 `Exporter`、schema version、manifest 和 capability negotiation 文档。
- [ ] Step 2: 为第三方 exporter 定义进程边界、输入目录、输出 manifest 和错误 JSON。
- [ ] Step 3: 测试未知 schema、能力缺失、输出路径越界和插件崩溃隔离。
- [ ] Step 4: 提交 `docs: publish adapter extension contract`。

### Task 23: 安全、性能和跨平台发布门禁

**Files:**
- Create: `tests/security/path_escape.rs`
- Create: `tests/security/secret_leak.rs`
- Create: `tests/integration/full_journey.rs`
- Create: `benchmarks/capture_compile.rs`
- Create: `SECURITY.md`
- Modify: `README.md`

- [ ] Step 1: 写完整 journey：capture → compile → lint → verify → receipt → export。
- [ ] Step 2: 增加路径逃逸、命令注入、网络绕过、环境泄漏、receipt 秘密泄漏和后台进程测试。
- [ ] Step 3: 为 10k Tape events、100-step Workflow 和 1GB 日志设置可观测 benchmark，不以未定义硬阈值阻断功能测试。
- [ ] Step 4: macOS/Linux CI 运行完整矩阵；记录 PTY/file watcher 的平台差异。
- [ ] Step 5: 发布 `SECURITY.md`、漏洞披露流程、兼容性表和版本策略。
- [ ] Step 6: 提交 `test: add full product security and release gates`。

## 最终验收矩阵

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

完整产品完成条件：

- Capture 产生可恢复、可脱敏、可审查的 Tape。
- Compiler 在无模型模式下确定性生成可 lint Skill；模型 proposal 只能增强描述，不能绕过 policy。
- Replay 在临时工作区中执行并可取消、超时、拒绝越权。
- Verify 生成可比较 Receipt，指出具体步骤和断言差异。
- Generic 与至少一个平台 exporter 可通过相同的 lint/verify 门禁。
- Console 只读展示 timeline、Workflow/权限 Diff、运行状态和 Receipt。
- 完整主链路无需云服务，且新用户可在 5 分钟内完成第一条通过验证的 Skill。
