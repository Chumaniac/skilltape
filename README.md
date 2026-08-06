# SkillTape

SkillTape 是一个 local-first、可回放验证的 Agent Skill 编译器。它把一次真实的终端和文件工作流记录为可脱敏、可审查的 Tape，再确定性编译成可回放、可验证、可提交到 GitHub 的 Skill 包。

核心闭环是：

```text
Capture → Tape → Compile → Lint/Policy → Replay → Verify/Receipt → Export
```

核心运行时不需要云服务或模型 provider。模型 proposal 只能补充描述，不能绕过 schema、权限或 policy 门禁。

## 五分钟本地试跑

需要 Rust stable；Linux 上执行 Replay/Verify 还需要 `bubblewrap`，macOS 使用系统的 `/usr/bin/sandbox-exec`。源码构建方式和预构建 release 安装方式见[安装指南](docs/guides/installation.md)。

```bash
git clone <your-skilltape-repository>
cd skilltape

# 构建 CLI；核心命令不需要 Node.js 或云服务
cargo install --path crates/skilltape-cli

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

`capture --yes` 是明确的本地确认；Capture 默认不保存原始秘密环境变量，Tape 输出也应视为本地敏感工件。提交仓库前请检查 `.gitignore`、Tape、Receipt 和导出目录。

## Console

Console 是可选的只读本地查看器，展示 Capture 时间线、Workflow/权限 Diff、运行状态和 Receipt。它不会在浏览器中执行命令，也不会修改 workspace。

从源码运行 Console 还需要构建 UI 和 API companion binary：

```bash
npm --prefix apps/skilltape-console install
npm --prefix apps/skilltape-console run build
cargo build --release -p skilltape-cli -p skilltape-console-api

./target/release/skilltape console --workspace .
```

CLI 默认只绑定 `127.0.0.1`，退出时会回收 API 子进程；如需自动打开浏览器，追加 `--open`。UI 未构建时，Console 会给出明确错误，不会伪装成已启动。

## CLI 命令

| 命令 | 作用 |
| --- | --- |
| `skilltape init <name> --output <dir>` | 创建最小 Skill 包模板 |
| `skilltape lint <skill> [--strict] [--json]` | 校验 schema、路径、权限、policy 和 lockfile |
| `skilltape capture <name> --workspace <dir> --command <program> --yes` | 记录终端和文件变化为 Tape |
| `skilltape compile <tape> --output <dir>` | 无模型确定性编译 Skill 包 |
| `skilltape replay <skill> [--input <json>]` | 在隔离临时工作区中回放并输出脱敏摘要 |
| `skilltape verify <skill> [--receipt <json>] [--json]` | 回放、执行断言并生成 Receipt |
| `skilltape export <skill> --target <target> --output <dir>` | 通过 lint 门禁导出通用或平台包 |
| `skilltape console [--workspace <dir>] [--port <port>] [--open]` | 启动只读本地 Console |

所有命令都可以通过 `cargo run -p skilltape-cli -- ...` 从源码执行。失败的输入通常返回 code 2，policy/export/verify 失败返回非零 code；CI 应断言失败而不是忽略它。

## CI 与 Skill 仓库集成

- `.github/workflows/ci.yml` 运行 fmt、Clippy、workspace tests、有效 example lint 和无效 fixture 的预期失败断言。
- `.github/workflows/skill-verify.yml` 是只运行本地 CLI 的模板，不上传 Tape、Receipt、日志或秘密。
- release 安装脚本要求固定版本、下载 checksum 并在校验成功前不替换已有 binary；具体参数见[安装指南](docs/guides/installation.md)。

## 设计目标

- 本地优先，不强制云端服务。
- LLM 只能生成受约束的结构化 Workflow IR，不能直接执行任意 Shell。
- 默认拒绝未声明的文件、网络、进程和秘密访问权限。
- 通过 fixtures、受控回放和 Receipt 证明 Skill 的执行结果。
- 使用 Git、JSON/YAML 和适配器连接不同 Agent 平台。

## 文档

- [安装、release 和 GitHub Actions](docs/guides/installation.md)
- [完整产品设计](docs/superpowers/specs/2026-08-05-skilltape-full-product-design.md)
- [实现计划](docs/superpowers/plans/2026-08-05-skilltape-full-product.md)
