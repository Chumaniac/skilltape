# SkillTape 完整产品设计规格

> 状态：待用户审阅
> 日期：2026-08-05
> 产品形态：local-first CLI/SDK 为核心，Web Console 为可选本地配套

## 1. 产品定义

SkillTape 是一个把“人完成的一次真实工作”转换成“可审计、可回放、可分享的 Agent Skill”的本地优先工具链。

完整产品的核心闭环是：

```text
Capture → Tape → Compile → Policy/Lint → Replay → Verify/Receipt → Export
```

核心承诺：

> 一次真实操作，生成一个有证据、能回放、可提交到 GitHub 的 Agent Skill。

SkillTape 的核心能力必须在本地完成，不依赖云端账号或固定模型供应商。模型只能帮助解释 Tape、提出结构化 Workflow；模型输出不能绕过 Schema、权限策略或受控 Runner。

## 2. 产品边界

### 2.1 完整产品包含

- Rust CLI/SDK：安装、捕获、编译、审查、回放、验证、导出。
- Tape：终端、文件变化、环境上下文和权限决策的结构化记录。
- Compiler：Tape 到 Workflow IR、Skill 文档和权限清单的编译器。
- Policy Engine：路径、进程、网络、环境变量和秘密访问策略。
- Replay Runner：临时工作区、进程监督、超时、取消和输出采集。
- Verify：fixtures、断言、回放对比和 Receipt 生成。
- Export Adapter：通用 Skill 包，以及少量高价值 Agent 平台适配器。
- Local Console：可选的时间线、Diff、权限审查和 Receipt 查看器。
- GitHub-ready 项目结构：可直接审查、Fork、测试和发布。

### 2.2 明确不做

- 不提供云端 Skill 市场、账号和计费系统。
- 不把 SkillTape 变成通用桌面 RPA 或任意 GUI 自动化工具。
- 不允许默认执行任意 `sh -c`、绝对路径、隐式环境变量或未声明网络访问。
- 不绑定单一 LLM、Agent 平台或托管服务。
- 不在第一版承诺 Windows 原生 Capture；Windows 通过 WSL 或后续适配器接入。

## 3. 用户与端到端流程

### 3.1 主要用户

第一用户是开发者和 Agent 工作流作者：他们希望把一次成功的终端工作沉淀成可复用 Skill，并用证据证明它没有扩大权限或依赖不可见的本机状态。

第二用户是高级自动化用户：他们通过 CLI 或本地 Console 捕获 PDF、文档、数据清洗和文件整理流程，不需要理解 Rust 或内部 IR。

### 3.2 CLI 主流程

```bash
skilltape capture pdf-to-study
skilltape compile .skilltape/tapes/tape_01H...
skilltape lint ./pdf-to-study
skilltape verify ./pdf-to-study
skilltape export ./pdf-to-study --target generic
```

完整产品的命令行为：

| 命令 | 作用 | 必须产生的结果 |
|---|---|---|
| `init` | 创建空 Skill 包 | 六个可 lint 文件 |
| `capture` | 记录一次受控人工工作 | 可重放的 Tape 目录 |
| `compile` | Tape 编译为 Skill | Workflow、权限、文档和 fixtures 草稿 |
| `lint` | Schema 与策略审查 | 稳定诊断、JSON 输出、明确退出码 |
| `replay` | 在临时工作区执行 | 运行日志和中间结果 |
| `verify` | 回放并执行断言 | Receipt 和通过/失败状态 |
| `export` | 生成平台包 | 可提交到 GitHub 的目标目录 |
| `console` | 启动本地查看器 | 时间线、Diff、权限和 Receipt 页面 |

## 4. 核心领域模型

### 4.1 Tape

Tape 使用版本化 JSONL 事件流，事件带稳定序号、相对时间、来源和敏感信息处理状态。最小事件集合：

- `session.started` / `session.finished`
- `terminal.command`：程序、参数、工作目录、退出码、stdout/stderr 引用
- `filesystem.created` / `modified` / `moved` / `deleted`
- `permission.requested` / `permission.decided`
- `environment.snapshot`：只允许显式白名单字段
- `capture.warning`：丢失事件、截断输出或脱敏提示

Tape 不保存默认的原始秘密。命令参数和输出在写入 JSONL 前经过规则脱敏；原始内容只有在用户明确选择的本地诊断目录中短期保存，并且不进入导出包。

### 4.2 Workflow IR

Workflow 是唯一可执行中间表示，沿用当前 `workflow.yaml` 的版本化 schema。第一版动作集合固定为 `exec`、`script`、`file`、`assert`。

每个步骤必须有稳定 `id`、输入引用、超时、输出声明和来源事件范围。Compiler 只生成结构化步骤；Runner 不解释 Markdown，也不直接执行模型生成的自然语言。

### 4.3 SkillPackage

SkillPackage 继续使用当前六个核心文件：

```text
skilltape.yaml
workflow.yaml
permissions.json
skilltape.lock
SKILL.md
README.md
```

完整产品额外增加可选目录：

```text
fixtures/
  input/
  expected/
  assertions.yaml
receipts/
tapes/
```

核心包必须保持 Git-friendly；每个生成字段都能通过 source path 或 Tape event id 追溯。

### 4.4 Run 与 Receipt

Run 是一次具体输入下的受控执行，默认运行在临时工作区。Receipt 至少记录：Skill 包哈希、Tape/Workflow 版本、环境摘要、每步状态、命令退出码、输入输出哈希、权限决策、断言结果、耗时、取消或失败原因。

Receipt 禁止写入秘密原文；日志采用引用、哈希和截断摘要。

## 5. 系统架构

### 5.1 Rust workspace

现有 `skilltape-schema`、`skilltape-core`、`skilltape-cli` 继续作为 foundation。完整产品按职责增加：

```text
crates/
  skilltape-schema/       版本化模型和 JSON Schema
  skilltape-core/         SkillPackage、诊断、模板和公共契约
  skilltape-tape/         Tape 事件模型、JSONL 存储、脱敏
  skilltape-capture/      PTY、命令捕获、文件监听、权限询问
  skilltape-compiler/     Tape → Workflow/Skill 的确定性编译
  skilltape-policy/       权限规则、风险分级、策略解释
  skilltape-runner/       临时工作区、进程监督、输出采集
  skilltape-verify/       fixtures、断言、Receipt、回放差异
  skilltape-export/       通用包与 Agent 平台适配器
  skilltape-cli/          命令解析、输出、配置和进程编排
```

每个 crate 必须有独立单元/集成测试；跨 crate 的行为通过稳定接口测试，不通过共享内部状态耦合。

### 5.2 可选 Local Console

Console 由本地 CLI 启动，读取同一工作区的 Tape、Skill 和 Receipt。第一版使用本地 HTTP 服务和 React/TypeScript 页面，不将 Console 作为 CLI/SDK 的运行时依赖。

Console 的只读页面顺序：

1. Capture timeline：事件、命令、文件变化和脱敏标记。
2. Compile review：Workflow 步骤、变量、来源事件和 Diff。
3. Permission review：进程、文件、网络、秘密访问及风险级别。
4. Verify run：步骤状态、日志摘要、断言和 Receipt。

## 6. 数据流与安全策略

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

安全规则：

- Capture 只监听用户明确选择的工作区和终端会话。
- 所有路径规范化为 workspace-relative；绝对路径、`..`、Windows drive/UNC 路径拒绝。
- `exec` 的 program 必须出现在 `permissions.json`，参数中的输入引用必须已声明。
- 网络默认关闭；host、方法和请求来源必须显式声明。
- 环境变量默认不读取；秘密只允许通过本地用户确认的引用传递，不写入 Tape/Receipt。
- Runner 使用临时目录、资源限制、超时和可取消 token；禁止无界后台进程。
- 每次策略拒绝都生成稳定诊断和可解释原因，不使用静默降级。

## 7. 完整实施阶段

### Phase 0：Foundation（已完成）

交付当前 `init`、`lint`、schema、package loader、诊断、示例、JSON 输出和基础 CI 门禁。

完成条件：foundation workspace 可独立构建，最小包能 lint，通过测试和 Clippy。

### Phase 1：Capture

交付终端 PTY 捕获、文件监听、事件 JSONL、实时脱敏、会话恢复和 Capture 权限提示。

完成条件：在 macOS/Linux 上捕获一个真实终端工作流，Tape 可重放读取，事件丢失和敏感字段有明确报告。

### Phase 2：Compiler

交付确定性 Tape 分析、变量抽取、Workflow 生成、权限推断、SKILL.md 草稿和人工 review manifest。

模型接入只作为可插拔建议器：模型输出先进入 proposal 文件，必须经过 schema、policy 和用户确认才可写入正式 Skill。

完成条件：同一 Tape 在无模型模式下产生稳定 Workflow；有模型时只改善命名、变量和说明，不改变安全边界。

### Phase 3：Replay、Verify、Receipt

交付临时工作区 Runner、命令/文件输出采集、超时取消、fixtures、断言、回放差异和 Receipt。

完成条件：编译出的 Skill 在干净临时目录中可重复回放；未声明权限必定拒绝；相同输入的 Receipt 可比较。

### Phase 4：Export 与适配器

先交付通用 Agent Skill 导出，再按真实用户量增加平台适配器。适配器只负责文件布局、元数据和调用约定，不复制核心执行逻辑。

完成条件：导出目录可直接提交到 GitHub，并通过本仓库的 lint/verify；任意适配器失败不影响通用格式。

### Phase 5：Local Console

交付只读 timeline、Workflow Diff、权限 Diff、Verify run 和 Receipt 页面；Console 通过版本化本地 API 读取工件，不拥有第二套业务逻辑。

完成条件：用户不打开终端也能审查一次 Capture、确认权限、查看失败步骤和下载 Receipt。

### Phase 6：发布与生态

交付安装脚本、跨平台 release、文档站、贡献模板、fixture/adapter 插件接口、GitHub Actions 模板和安全披露流程。

完成条件：新用户 5 分钟内完成 Capture → Compile → Verify；第三方可以只通过公开接口贡献适配器或断言。

## 8. 测试与质量门禁

每个阶段必须同时具备：

- 单元测试：纯函数、schema、脱敏、策略和状态转换。
- 集成测试：crate 间 Tape/Workflow/Receipt 契约。
- CLI 测试：成功、拒绝、取消、JSON 输出和稳定退出码。
- Fixture 回放：稳定输入、预期文件哈希、预期诊断。
- 安全测试：路径逃逸、命令注入、秘密泄漏、未声明权限、网络绕过和后台进程。
- 跨平台测试：macOS/Linux；PTY 和文件监听采用适配层隔离。

合并门禁：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、CLI smoke、fixture verify 和秘密扫描全部通过。

## 9. 完整产品验收标准

完整产品只有在以下链路全部通过时才算完成：

1. 用户可以在本地捕获一个真实终端/文件工作流。
2. Tape 可以脱敏、保存、恢复和审查。
3. Compiler 可以生成确定性 Workflow、权限清单、Skill 文档和 fixtures。
4. Schema/Policy 可以阻止路径、进程、网络和秘密越权。
5. Runner 可以在临时工作区回放并支持取消、超时和失败恢复。
6. Verify 可以生成可比较的 Receipt，指出具体失败步骤和差异。
7. Export 可以生成通用 Skill 包和至少一个经过测试的平台适配器。
8. Console 可以读取本地工件完成时间线、权限、Diff 和 Receipt 审查。
9. 不配置云端服务也能完成主链路；模型是可选增强而非硬依赖。
10. 新用户能够按 README 在 5 分钟内完成第一条通过验证的 Skill。

## 10. 关键决策

- CLI/SDK 是产品核心，Console 不得成为核心能力的隐性依赖。
- 先完成单条安全、可回放的垂直链路，再扩展平台和 UI。
- JSON Schema、JSONL、YAML、Markdown 和 Receipt 都是公开、可审查、可版本化的文件契约。
- 所有模型能力都必须位于确定性 Compiler/Policy 之后的建议层。
- 完整产品的第一个商业/传播闭环是“真实操作 → 可验证 Skill → GitHub 可分享”，而不是云端市场。
