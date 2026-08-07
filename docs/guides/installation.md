# 安装与本地 CI

SkillTape 有两种安装方式：从源码构建，或从 GitHub Release 下载已校验的目标平台 binary。核心 CLI 不依赖云服务；Console 需要同时提供 `skilltape-console-api` binary 和已构建的 Vite `dist`。

## 从源码构建

安装 Rust 1.97.1（仓库中的 `rust-toolchain.toml` 会自动选择该版本）后，在仓库根目录运行：

```bash
cargo build --locked --release -p skilltape-cli -p skilltape-console-api
cargo install --locked --path crates/skilltape-cli
```

Replay/Verify 还会启动受限的本地执行器：Linux 需要 `bwrap`（Debian/Ubuntu 可安装 `bubblewrap`），macOS 需要系统提供的 `/usr/bin/sandbox-exec`。没有对应 sandbox 时，Capture、Compile、Lint 和 Export 仍可使用，但 Replay/Verify 会安全地失败并提示环境不可用。

只使用 Capture、Compile、Lint、Replay、Verify 或 Export 时，安装 `skilltape` 即可。要从源码启动 Console：

```bash
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
./target/release/skilltape console --workspace .
```

开发时可用 `npm --prefix apps/skilltape-console run dev` 查看 UI，但 API 仍应通过 `skilltape-console-api` 或 `skilltape console` 提供；浏览器端没有执行或写入能力。

## Release 安装

Release 资产采用以下命名：

```text
skilltape-v<version>-<target>.tar.gz   # macOS/Linux
skilltape-v<version>-<target>.zip      # Windows
checksums.txt
```

每个 archive 的内容为：

```text
skilltape-v<version>-<target>/
├── skilltape
├── skilltape-console-api
└── console/
    ├── index.html
    └── assets/
```

安装后，CLI 和 API companion 位于安装目录，`console/` 位于安装目录的父目录。例如默认 Unix 目录为 `$HOME/.local/bin/skilltape`、`$HOME/.local/bin/skilltape-console-api` 和 `$HOME/.local/console/index.html`。安装器会先下载、校验 checksum、检查三类资产并完成 staging，再执行替换；下载、校验、解压或 staging 失败时不会覆盖已有 CLI。

当前安装脚本要求显式设置 release 下载根地址，避免在仓库 owner、镜像或私有 release 未确定时误下到错误项目：

```bash
export SKILLTAPE_RELEASE_BASE_URL="https://github.com/<owner>/<repo>/releases/download"
SKILLTAPE_VERSION=0.1.0 ./scripts/install.sh
```

也可以把版本作为第一个参数，并覆盖安装目录和 target：

```bash
SKILLTAPE_RELEASE_BASE_URL="https://github.com/<owner>/<repo>/releases/download" \
  ./scripts/install.sh 0.1.0 "$HOME/.local/bin" "aarch64-apple-darwin"
```

Windows PowerShell：

```powershell
$env:SKILLTAPE_RELEASE_BASE_URL = "https://github.com/<owner>/<repo>/releases/download"
$env:SKILLTAPE_VERSION = "0.1.0"
.\scripts\install.ps1
```

Release workflow 位于 `.github/workflows/release.yml`，由 `v*` tag 或手动版本输入触发，覆盖 Linux x86_64、macOS x86_64/arm64 和 Windows x86_64。当前工作树没有配置 Git remote；启用发布前需要先配置目标 GitHub repository、tag 策略和发布权限。workflow 不上传 Tape、Receipt、日志、环境变量或 provider credential。

脚本会下载 archive 和 `checksums.txt` 到随机临时目录，比较目标 asset 的 SHA-256，校验 archive 中的 CLI、API companion 和 Console UI，并在全部资产 staging 后替换目标文件。下载失败、checksum 不匹配、archive 中缺少资产或权限失败时，已有 binary 不会被覆盖。版本、下载根地址和 target 都可以固定，脚本不会读取或写入 token、cookie、环境秘密或项目 `.env`。

## Capture → Compile → Verify

下面的示例使用临时 workspace，避免把 Tape 或 Receipt 写进仓库：

```bash
demo_workspace="$(mktemp -d)"
skilltape capture demo \
  --workspace "$demo_workspace" \
  --command /bin/echo \
  --output "$demo_workspace/.skilltape/tapes/tape_demo" \
  --yes
skilltape compile "$demo_workspace/.skilltape/tapes/tape_demo" \
  --output "$demo_workspace/demo-skill"
skilltape lint "$demo_workspace/demo-skill" --strict
skilltape verify "$demo_workspace/demo-skill" \
  --receipt "$demo_workspace/receipt.json" \
  --json
skilltape export "$demo_workspace/demo-skill" \
  --target generic \
  --output "$demo_workspace/exported-skill"
```

`--command` 接收一个可执行程序名；需要参数时应先把它封装为受控脚本并在 Skill 的 permissions/workflow 中声明。不要把未审查的自然语言直接拼成 Shell 命令。

要捕获人工操作流程，可以省略 `--command`，Capture 会启动当前用户的 shell，并在输入 `exit` 后结束；若指定的程序本身需要从终端读取输入，则添加 `--interactive`。交互模式将实时终端输出发送到 stderr，避免污染 `--json` 的 stdout 摘要；Tape manifest 的 `id` 每次运行都会唯一生成。

## 本地验证

与 CI 一致的本地门禁（依赖锁文件，不接受隐式升级）：

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
cargo run --locked -p skilltape-cli -- lint examples/minimal-skill
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
npm --prefix apps/skilltape-console test
python3 scripts/test_release_package.py
python3 scripts/test_release_workflow.py
bash scripts/test_install.sh
```

无效 fixture 应明确失败（当前 policy code 为 3）：

```bash
if cargo run --locked -p skilltape-cli -- lint tests/fixtures/invalid-skill; then
  echo "invalid fixture unexpectedly passed" >&2
  exit 1
fi
```

## GitHub Actions

复制或启用 `.github/workflows/skill-verify.yml` 时，将 `skill_path` 指向仓库内已经审查的 Skill 目录。模板只 checkout 当前仓库、构建本地 CLI、运行 `lint`；它没有 artifact upload、Tape/Receipt 上传、远程 provider 或 secret dump 步骤。若要在 CI 生成 Receipt，请把它留在 runner 临时目录并显式清理，除非仓库另有经过审查的发布策略。
