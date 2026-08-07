# SkillTape Security

SkillTape 默认把工作流当作不可信输入，把执行当作高风险边界。安全目标是：在本地记录和回放 Agent Skill 时，限制文件、进程、网络和环境访问，并让 Tape、Receipt、日志和导出物不携带秘密明文。

## 威胁模型与边界

SkillTape 保护的对象包括：

- 工作区之外的文件和目录；
- 未在 permissions 中声明的可执行程序、网络主机和环境变量；
- 命令注入、路径遍历、符号链接越界和后台子进程残留；
- Receipt、Capture 输出和环境快照中的秘密明文。

Replay/Verify 由平台受限执行器承载：Linux 使用 `bubblewrap`，macOS 使用 `/usr/bin/sandbox-exec`。CLI 还会在策略层拒绝危险命令、越界路径、关闭的网络和秘密环境标识符。受限执行器是纵深防御的一部分，不应被理解为允许用户执行任意未审查代码。

以下事项不属于安全边界的替代品：

- 用户主动授予的权限可能允许 Skill 读取或修改该权限范围内的数据；
- 已被用户信任的宿主、内核、操作系统或第三方二进制的漏洞不由 SkillTape 单独修复；
- 不支持的操作系统不会被模拟成安全隔离环境，Replay/Verify 应明确失败。

## 秘密处理

Capture 默认不读取环境变量值，只记录显式 allowlist 中变量的名称、长度和 SHA-256 元数据。终端输出会在持久化前执行命名秘密、常见 token 和配置模式脱敏。Receipt 和 Replay 摘要只保留输出摘要、长度和策略决定，不保留 stdout/stderr 原文。

Tape、Receipt、导出目录、临时日志和 CI 工作区仍可能包含路径、命令名、文件大小和其他敏感元数据。不要把未经审查的这些工件上传到公开仓库或 CI artifact；不要在测试、Issue、提交信息或日志中写入真实凭据。

## 平台兼容性

| 平台 | Capture / Compile / Lint / Export | Replay / Verify | 受限执行器与差异 |
| --- | --- | --- | --- |
| macOS | 支持 | 需要 `/usr/bin/sandbox-exec` | PTY 使用系统实现；文件监听使用平台 watcher；sandbox profile 只开放临时工作区 |
| Linux | 支持 | 需要 `bwrap`/`bubblewrap` 和可用的 user namespace | `bwrap --unshare-all` 隔离网络、环境和文件系统；CI 会预装 bubblewrap |
| Windows | 支持非执行型命令和包操作 | 当前明确拒绝并返回 sandbox unavailable | 发布包可使用 Windows 安装脚本；等价受限执行器接入前不宣称 Replay/Verify 支持 |

完整发布门禁只在 Linux 和 macOS 矩阵上执行。PTY 的终端大小、信号语义以及文件 watcher 的事件合并顺序可能有平台差异；Tape schema 和 Receipt schema 不应依赖这些非确定性字段的具体时间值。

## 漏洞披露

请不要通过公开 Issue 发布可利用的 sandbox escape、路径越界、秘密泄漏或远程执行细节。优先使用仓库 GitHub Security 页面创建 Private Vulnerability Report；如果该入口不可用，请先提交不含利用细节的维护者私信，并等待安全响应后再公开。

报告应包含：受影响版本和平台、最小复现步骤、预期与实际行为、是否需要特定 permissions、是否能读取工作区外文件或泄漏秘密，以及不会包含凭据的日志或补丁。请先删除真实 token、cookie、私钥和生产数据。

安全修复会在确认影响范围后通过受保护的提交和 release note 发布；修复版本是否 backport 取决于受影响版本仍处于支持窗口的情况。

## 版本与兼容性策略

- CLI 和公共 schema 遵循 SemVer；当前开发线为 `0.x`，小版本仍可能调整实验性 CLI 行为。
- Tape、Receipt、Run、Plugin Export 协议使用显式的 `skilltape.dev/.../v1` schema 标识；不兼容改变应创建新版本标识并保留旧版本的读取路径，或在 release note 中明确迁移要求。
- 权限默认值、sandbox 配置和秘密脱敏规则属于安全行为。安全收紧可以在补丁版本发布；放宽默认权限必须经过独立审查并记录迁移影响。
- GitHub Actions 只运行本地代码和受审查 fixture，不上传 Tape、Receipt、日志或环境快照。

## 本地安全门禁

```bash
cargo test -p skilltape-cli --test security_path_escape --test security_secret_leak --test integration_full_journey -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo bench -p skilltape-cli --bench capture_compile
SKILLTAPE_BENCHMARK_LARGE=1 cargo bench -p skilltape-cli --bench capture_compile
```

基准命令不定义未经校准的性能通过线；它用于观察回归趋势。大日志场景使用稀疏文件并按需顺序读取，仍应在专用 runner 上运行。
