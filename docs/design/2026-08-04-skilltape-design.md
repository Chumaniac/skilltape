# SkillTape 完整设计方案

> 状态：设计稿 v0.1  
> 日期：2026-08-04  
> 项目：独立开源仓库，不依赖 GenkoyAI  
> 工作名称：SkillTape

## 1. 摘要

SkillTape 是一个 local-first、可回放验证的 Agent Skill 编译器。

它捕获用户真实完成的一次终端和文件工作流，将其编译成一个可审计、可复用、可提交到 GitHub 的 Agent Skill 包。Skill 包同时包含：

- 面向 Agent 的 `SKILL.md`
- 面向运行时的 `workflow.yaml`
- 面向安全审查的 `permissions.json`
- 面向验证的 fixtures、断言和 Receipt

核心产品承诺：

> 一次真实操作，生成一个有证据、能回放、可分享的 Agent Skill。

SkillTape 不把 LLM 当作直接执行器。LLM 只能帮助理解轨迹和生成结构化 Workflow IR；所有实际命令、文件访问和网络访问都必须经过 Schema、权限策略和回放器。

## 2. 研究依据与产品判断

本方案参考 2026-08-04 GitHub Trending 日榜、周榜和月榜：

- [GitHub 日榜](https://github.com/trending?since=daily)
- [GitHub 周榜](https://github.com/trending?since=weekly)
- [GitHub 月榜](https://github.com/trending?since=monthly)

当前可见趋势显示，Agent Skills、模型网关、Agent 记忆、浏览器/桌面操作、本地推理和多模态工具的增长集中度较高。由此得到的产品选择是：

1. 以开发者工具和 Agent 基础设施作为早期 Star 的主要入口。
2. 用可视化轨迹、Diff 和 Receipt 形成普通用户也能理解的演示体验。
3. 以本地运行、开源格式和多平台适配避免供应商锁定。
4. 把“可验证”作为区别于普通 Prompt/Skill 生成器的核心价值。

这只是提高早期传播概率的设计判断，不保证任何具体 Star 数量。

## 3. 产品定位与目标用户

### 3.1 主要用户

第一用户群是开发者和 AI 工作流作者：

- 为 Claude Code、Codex、Cursor、Cline 或其他 Agent 编写 Skill
- 不想手写复杂工作流说明
- 需要验证 Skill 是否真的有效
- 希望把个人经验发布成可 Fork 的开源项目

第二用户群是高级非开发者：

- 批量整理文件
- 处理 PDF 和文档
- 生成报告
- 清洗数据
- 复用办公流程

普通用户不需要理解内部 IR；他们可以通过本地 Web UI 完成录制、审查和执行。

### 3.2 核心问题

目前的 AI Skill/Agent 工作流通常存在四个问题：

1. Skill 主要是一段文档，无法证明实际可执行。
2. 生成的命令可能扩大文件、网络或进程权限。
3. 工作流依赖作者本机环境，难以复现。
4. 其他人很难通过 GitHub Review 判断 Skill 是否可信。

SkillTape 的解决方式是把一次工作拆成：

```text
操作轨迹 → 结构化工作流 → 权限策略 → 回放结果
```

### 3.3 非目标

MVP 不做：

- 云端 Skill 市场
- 用户账号和计费
- 自建大模型
- 多 Agent 协同编排
- 全功能桌面 RPA
- 一开始支持任意 GUI 操作
- 一开始兼容所有 Agent 平台
- 允许第三方 Skill 默认执行高风险系统操作

## 4. 产品契约与核心对象

SkillTape 只保留四个核心对象：

| 对象 | 含义 |
|---|---|
| Tape | 一次真实操作过程的结构化记录 |
| Skill | 从 Tape 编译出的可复用工作流包 |
| Run | Skill 的一次实际执行 |
| Receipt | Run 的证据、日志和结果摘要 |

### 4.1 Tape

Tape 不是简单视频，而是带顺序号的事件记录，包含命令、输出、工作目录、文件变化和权限决策。

### 4.2 Skill

Skill 是可提交到 Git 的目录，包含文档、Workflow IR、权限清单、fixtures 和测试。

### 4.3 Run

Run 是某个 Skill 在一组输入上的执行实例。Run 默认使用临时工作区，不直接修改用户原始目录。

### 4.4 Receipt

Receipt 记录每一步的状态、耗时、输入输出哈希、权限决策、断言结果和失败原因。

## 5. 用户流程

```text
Capture
  捕获一次成功的人工工作

Compile
  将轨迹编译为 Workflow IR 和 Skill 文档

Review
  查看步骤、变量、文件访问和权限 Diff

Verify
  使用 fixtures 在临时环境中回放

Share
  导出为可提交、可 Fork 的 GitHub 项目
```

理想命令流：

```bash
skilltape capture pdf-to-study
skilltape compile tape_01H...
skilltape lint ./pdf-to-study
skilltape verify ./pdf-to-study
skilltape export ./pdf-to-study
```

## 6. MVP 边界与成功标准

### 6.1 MVP 必须包含

- 终端命令捕获
- 文件创建、修改、移动和删除捕获
- 事件即时脱敏
- 本地模型和 OpenAI-compatible provider
- Tape 到 Workflow IR 的编译
- `SKILL.md` 生成
- `workflow.yaml` 生成
- `permissions.json` 生成
- Schema 和策略检查
- fixtures 驱动的回放验证
- Receipt 生成
- 本地 Web 查看器
- 通用 Skill 包导出

### 6.2 平台边界

- 第一阶段支持 macOS 和 Linux
- Windows 后续通过 WSL 或独立适配器支持
- 第一阶段只捕获终端和指定工作区文件变化
- 浏览器捕获属于第二阶段
- 本地 Web UI 不采用 Tauri/Electron，先由本地服务托管

### 6.3 成功标准

用户从安装到生成第一个通过验证的 Skill，应在 5 分钟内完成。

MVP 验收条件：

1. 用户可以捕获一个真实工作流。
2. 编译器生成可读、可校验的 Skill 包。
3. 未声明的命令、路径和网络访问会被阻止。
4. Skill 可以在 fixtures 中重复回放。
5. 回放会生成 Receipt。
6. 生成目录可以直接提交到 GitHub。
7. 不配置云端服务也能完成完整流程。

## 7. 总体架构

### 7.1 技术选择

推荐采用 Rust 核心加 TypeScript/React 本地控制台：

- Rust 负责单二进制分发、PTY、文件监听、进程监督和运行时策略。
- TypeScript/React 负责时间线、Diff、权限审查和 Receipt 展示。
- Axum 提供本地 HTTP API 和 SSE。
- JSON Schema 是跨 Rust/TypeScript 的契约来源。
- JSONL、YAML 和 Markdown 作为可审查的文件格式。

建议技术组件：

| 能力 | 选择 |
|---|---|
| 异步运行时 | Tokio |
| PTY | portable-pty |
| 文件监听 | notify |
| HTTP | Axum |
| 序列化 | serde / serde_json / serde_yaml |
| Schema | JSON Schema + schemars |
| 日志 | tracing |
| Web UI | React + Vite + TypeScript |
| CI | GitHub Actions |

### 7.2 组件图

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

CLI 和 Web UI 必须复用同一套 Core，不在前端重新实现编译、策略或执行逻辑。

## 8. Capture Engine

### 8.1 终端捕获

`skilltape capture <name>` 启动一个受控 PTY Shell。用户在该 Shell 中正常完成工作，输入 `exit` 后结束捕获。

捕获内容：

- 命令输入
- stdout/stderr
- 退出码
- 当前工作目录
- 命令耗时
- 子进程摘要
- 文件变化

第一版不做全系统 Hook，也不试图记录用户在所有应用中的任意操作。

### 8.2 文件变化捕获

用户必须指定工作区。Capture Engine 使用初始快照加文件监听记录：

- 创建
- 修改
- 移动
- 删除
- 文件大小
- 前后哈希

文件内容默认不全部复制。只有用户指定的输入样例和输出样例进入 `fixtures/`。

### 8.3 事件格式

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

事件类型：

- `session.started`
- `command.started`
- `command.output`
- `command.finished`
- `file.changed`
- `approval.changed`
- `session.ended`

### 8.4 脱敏

脱敏发生在事件进入持久化层之前：

- 环境变量只记录名称，不记录值
- Token、Cookie、Authorization、私钥和常见 API Key 进行模式识别
- 命令参数中的疑似秘密替换为 `<REDACTED>` 标记
- 脱敏不确定时阻止导出
- 原始事件默认不保存

## 9. Tape 存储

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

设计原则：

- `events.jsonl` 追加写入
- 事件使用递增序号
- 中途崩溃也能保存为 `partial` Tape
- Tape 不等于 Skill，必须经过编译和审查
- 原始轨迹不进入 GitHub 包，除非用户明确选择

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

识别：

- 固定命令
- 可参数化路径
- 输入和输出文件
- 可合并步骤
- 不稳定数据
- 必要权限

### 10.2 LLM 边界

LLM 只能生成或修改 Workflow IR，不能：

- 直接执行 Shell
- 直接修改用户文件
- 自动批准权限
- 添加未观察到的程序而不提示
- 通过字符串拼接生成任意 Shell

### 10.3 编译失败

如果 Schema、变量、路径或权限校验失败，只生成诊断报告，不生成可运行 Skill。

## 11. Skill 包格式

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

### 11.3 Action 类型

MVP 只支持：

- `exec`：执行明确程序和参数
- `script`：执行包内、带哈希的辅助脚本
- `file`：受限复制、移动和创建目录
- `assert`：检查文件、JSON Schema、哈希或退出码

不允许使用任意 `sh -c` 作为规范化执行形式。

### 11.4 路径和变量规则

- 输入必须预先声明
- 路径必须是工作区相对路径
- 禁止绝对路径
- 禁止隐式环境变量
- 禁止自由字符串拼接 Shell
- 输出必须由步骤声明
- 步骤只能读取已声明输入或前序输出

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

默认拒绝未声明能力。

### 11.6 `skilltape.lock`

`skilltape.lock` 由 `verify` 生成，用来固定验证环境和包内脚本哈希。它不能包含 API Key、Token 或用户文件内容。

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

当本机工具版本与 lock 文件不一致时，`verify` 默认给出警告；`--strict` 模式直接失败。

### 11.7 `SKILL.md` 和 `README.md`

`SKILL.md` 面向 Agent，`README.md` 面向 GitHub 用户。两者都允许人工编辑。

SkillTape 只更新以下标记区块：

```markdown
<!-- skilltape:generated:start -->
Generated verification summary...
<!-- skilltape:generated:end -->
```

这样重新编译不会覆盖用户自己写的说明。

## 12. Policy Engine 与 Replay Runner

### 12.1 权限策略

默认策略：

- 工作区之外禁止读写
- 网络默认关闭
- 禁止读取环境变量秘密
- 禁止提权
- 只能执行声明过的程序
- 权限变更必须经过用户确认
- 未观察过的命令默认阻止

### 12.2 回放流程

```text
fixtures/input
    ↓
复制到临时工作区
    ↓
Policy Engine 检查
    ↓
Replay Runner 执行
    ↓
文件和断言检查
    ↓
生成 Receipt
```

### 12.3 运行后端

MVP 提供：

1. `guarded-local`：临时工作区、路径检查、进程监管、超时和网络策略。
2. `container`：Docker/Podman 可用时提供更强隔离。

`guarded-local` 不宣称是完整安全沙箱。执行不可信第三方 Skill 时必须推荐容器后端。

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

Receipt 包含：

- Skill 版本
- 输入文件哈希
- 每一步开始和结束时间
- 命令退出码
- 权限批准记录
- 文件变化
- 断言结果
- 失败步骤和原因
- 模型 provider 和模型名称
- 是否使用本地模型

成功必须明确列出完成步骤和断言；失败必须停止在具体步骤，不返回模糊成功。

## 14. CLI 设计

### 14.1 命令

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

### 14.2 通用选项

```text
--json
--verbose
--no-color
--workspace <path>
--yes
--dry-run
```

### 14.3 安全默认值

- `run` 默认要求权限确认
- `export` 默认要求通过 lint 和 verify
- `--approved` 只对已审查且权限未变化的 Skill 有效
- `--force` 不跳过策略检查，只允许覆盖生成文件

### 14.4 退出码

| 退出码 | 含义 |
|---:|---|
| 0 | 成功 |
| 1 | 参数或用户输入错误 |
| 2 | Schema 或编译错误 |
| 3 | 权限策略阻止 |
| 4 | 回放验证失败 |
| 5 | 本地环境或模型不可用 |

## 15. 本地 Web UI

### 15.1 页面

#### Dashboard

展示 Tape、Skill、最近验证结果、失败任务和未审查权限。

#### Tape Inspector

展示命令时间线、stdout/stderr、文件 Diff、脱敏标记和网络访问。

#### Compile Review

展示生成的步骤、输入输出、程序和权限 Diff。用户逐项确认，不提供默认的“允许全部”。

#### Skill Review

并列查看：

- Skill Overview
- Workflow
- Permissions
- Generated Docs

#### Verify Run

通过 SSE 显示每一步实时状态。

#### Receipt Viewer

以报告方式展示完整执行凭证，并支持复制 Markdown 摘要。

### 15.2 本地 API

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

### 15.3 UI 安全

- 只监听 `127.0.0.1`
- 每次启动生成随机访问 Token
- 检查请求 Origin
- 设置严格 CSP
- UI 不能直接执行 Shell
- 所有动作必须通过 Core API

### 15.4 首个演示流程

```text
capture → 时间线 → compile → 权限 Diff → verify → 绿色 Receipt → GitHub 包
```

这是 README 首屏 GIF 和公开演示视频的主流程。

## 16. Provider、Capture Source 和 Export Adapter

### 16.1 Provider

Provider 只负责结构化生成和能力检测：

```text
health()
complete_structured(schema, context)
capabilities()
```

首批实现：

- Ollama
- LM Studio
- OpenAI-compatible HTTP API

所有发送给 provider 的内容都必须是脱敏后的 Tape 摘要。`--offline` 模式禁止任何网络请求。

### 16.2 Capture Source

MVP 内置 `shell` Capture Source。后续扩展：

- browser
- git
- editor
- desktop

Capture Source 输出统一事件，不改变 Compiler 和 Policy Engine。

### 16.3 Export Adapter

核心始终输出 generic Skill 包。平台适配器独立实现：

```text
adapter id
protocol version
detect()
validate(skill)
render(skill, output_dir)
```

适配器建议以独立进程通过 JSON-RPC over stdio 工作，避免插件崩溃影响核心运行时。

首批目标：

- generic-agent-skill
- claude
- codex
- cursor

MCP 暴露和浏览器 Capture Adapter 放入后续版本。

## 17. Skill 分享机制

### 17.1 MVP

Git 仓库是第一注册表，不建设中心化市场。

```bash
skilltape export ./pdf-to-study
git add .
git commit -m "feat: add pdf to study skill"
```

SkillTape 自动生成：

- README
- 安装说明
- 权限摘要
- 验证徽章摘要
- 示例输入输出

### 17.2 后续

- `skilltape pack`
- `skilltape install <git-url>`
- `skilltape verify --strict`
- GitHub Action
- Skill 索引页
- 签名和 provenance

不在 MVP 中引入账号、云端运行和收费市场。

## 18. 安全与威胁模型

| 威胁 | 防御 | 残余风险 |
|---|---|---|
| 捕获时记录 Token | 流式脱敏、环境变量不落盘 | 模式无法识别的新型秘密 |
| 恶意 Skill 读用户文件 | 默认工作区限制、权限清单 | 本机模式不是硬隔离 |
| 网络数据外泄 | 默认关闭网络、记录网络策略 | 外部进程自身绕过需容器 |
| Prompt Injection | 内容按数据处理，LLM 不直接执行 | 外部 Agent 自身仍有风险 |
| Shell 注入 | `program + args[]`，禁止任意 `sh -c` | 被调用程序本身可能危险 |
| 浏览器跨站调用本地 API | 127.0.0.1、随机 Token、Origin 检查 | 用户主动暴露 Token 的风险 |
| 脚本篡改 | 脚本哈希和 lock 文件 | 用户主动覆盖文件 |

安全产品文案必须明确：

> SkillTape 提供默认拒绝和可审查执行；对于不可信 Skill，使用容器运行时，而不是把 guarded-local 当作完整沙箱。

## 19. 测试与验证策略

### 19.1 单元测试

- 事件序列化和排序
- 秘密脱敏
- 路径规范化
- 命令参数校验
- Schema 验证
- 权限匹配
- Receipt 生成
- JSON/YAML 往返

### 19.2 Golden Fixtures

首批固定示例：

1. `rename-images`
2. `pdf-to-markdown`
3. `csv-to-report`
4. `git-release-notes`
5. `workspace-cleanup`

每个示例都包含 Tape、Skill、fixtures、预期 Receipt 和失败用例。

### 19.3 安全测试

- 路径穿越
- 绝对路径
- Shell 注入
- 未声明可执行文件
- 未声明网络请求
- 环境变量秘密
- 超时和进程泄漏
- 恶意 YAML 字段
- 不可信脚本导入

### 19.4 集成和 E2E

- macOS/Linux CLI 集成测试
- Docker/Podman runner 测试
- Web UI Playwright 测试
- SSE 中断和重连
- Capture 中途退出恢复
- Provider 不可用时的明确失败

### 19.5 CI 门禁

Pull Request 必须通过：

- Rust format
- Rust lint
- TypeScript type-check
- 单元测试
- Golden fixture 回放
- 安全策略测试
- UI E2E
- 文档示例命令

## 20. 仓库结构

目标仓库结构：

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

共享 Schema 位于 `schemas/`，由 Rust 和 TypeScript 生成类型，避免两端漂移。

## 21. 实施阶段

### Phase 0：协议基础

- 初始化 Rust workspace
- 定义 JSON Schema
- 实现 Skill 包读写
- 实现 `init` 和 `lint`
- 添加第一个 Golden Fixture

完成标准：空 Skill 可以创建、校验和导出。

### Phase 1：Capture

- PTY Shell
- 命令事件
- 文件监听
- 事件脱敏
- Tape 持久化
- `capture` 和 `tapes` 命令

完成标准：能够捕获并重新打开一次完整 Tape。

### Phase 2：Compiler

- Trace Analyzer
- Provider 接口
- Workflow IR 生成
- 权限推导
- Skill 文档生成
- Compile Review 数据

完成标准：三个示例 Tape 能编译成合法 Skill。

### Phase 3：Verify

- Policy Engine
- guarded-local runner
- fixtures 回放
- 断言
- Receipt
- `verify` 和 `run`

完成标准：成功、失败、超时、权限阻止都能形成可读 Receipt。

### Phase 4：Console

- 本地 HTTP API
- SSE
- Dashboard
- Tape Inspector
- Compile Review
- Verify Run
- Receipt Viewer

完成标准：不使用 CLI 参数也能完成审查和验证。

### Phase 5：公开发布

- GitHub Releases
- macOS/Linux 预编译二进制
- Homebrew formula
- 示例 Skill 仓库
- 通用 Export Adapter
- GitHub Action 设计

## 22. Star 增长策略

项目不能依赖“功能很多”获取 Stars，而要降低第一次理解和第一次成功的成本。

首发仓库必须具备：

1. 首屏一句话解释产品。
2. 30–60 秒 GIF 展示 Capture → Verify。
3. 一个命令完成安装。
4. 三个可以复制的示例。
5. 明确的权限 Diff 和 Receipt 截图。
6. “No cloud required” 和 “No vendor lock-in” 的清晰说明。
7. GitHub Action/徽章的后续路线。
8. 适配器、示例和 fixtures 都可以独立贡献。

首发示例应优先选择可视觉化展示结果的工作流：

- PDF 转 Markdown
- 图片批量重命名和整理
- CSV 生成报告
- Git release notes 生成
- 项目目录清理

传播重点是“证据”和“可复现”，不是夸大 Agent 智能程度。

## 23. 主要风险与应对

### 风险一：成为普通 Skill 生成器

应对：把验证、权限、回放和 Receipt 作为产品核心，不能在 MVP 中删掉。

### 风险二：安全承诺过度

应对：明确区分 guarded-local 和 container；不把本机路径拦截宣传为完整沙箱。

### 风险三：跨平台范围过大

应对：先支持 macOS/Linux 的终端和文件工作流，浏览器和 Windows 后置。

### 风险四：LLM 供应商锁定

应对：Provider 只暴露结构化生成接口，默认支持 Ollama 和 OpenAI-compatible API。

### 风险五：格式过于复杂

应对：MVP 只保留四种 Action、线性步骤和 JSON Schema；条件、循环和多 Agent 放入后续版本。

### 风险六：UI 变成第二套系统

应对：UI 只调用 Core API，所有编译、策略、执行和 Receipt 逻辑在 Rust 核心中实现。

## 24. Definition of Done

SkillTape v0.1.0 只有在以下条件同时满足时才算完成：

- macOS/Linux 可以安装并启动 CLI
- 可以捕获终端和文件工作流
- Tape 可以恢复、查看和脱敏
- 可以通过 Provider 生成合法 Workflow IR
- LLM 不能绕过 IR 直接执行命令
- 权限默认拒绝
- 至少三个 Golden Fixture 通过回放
- 权限、路径、注入和秘密泄露测试通过
- 每次执行都有 Receipt
- 本地 Web UI 可以完成审查和验证
- 生成 Skill 可以不依赖 SkillTape 才能阅读
- README 有完整的 60 秒演示和失败案例
- 文档示例命令在干净环境中可运行

## 25. 最终设计结论

SkillTape 的核心不是“让 AI 自动做更多事情”，而是：

> 把一次成功的人类工作，编译成一个权限明确、结果可验证、可以被 Agent 复用的开源资产。

它以开发者基础设施获得早期传播，以普通用户可理解的视觉回放降低使用门槛，以 Git 和开放文件格式形成贡献生态。

MVP 必须坚持四个边界：

1. 终端和文件系统优先。
2. LLM 只生成结构化意图，不直接执行。
3. 默认拒绝权限，回放必须产生证据。
4. GitHub 包优先，云端市场后置。
