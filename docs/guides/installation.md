# 安装与本地 CI

SkillTape 有两种安装方式：从源码构建，或从 GitHub Release 下载已校验的目标平台 binary。核心 CLI 不依赖云服务；Console 需要同时提供 `skilltape-console-api` binary 和已构建的 Vite `dist`。

## 从源码构建

安装 Rust stable 后，在仓库根目录运行：

```bash
cargo build --release -p skilltape-cli -p skilltape-console-api
cargo install --path crates/skilltape-cli
```

只使用 Capture、Compile、Lint、Replay、Verify 或 Export 时，安装 `skilltape` 即可。要从源码启动 Console：

```bash
npm --prefix apps/skilltape-console install
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

脚本会下载 archive 和 `checksums.txt` 到随机临时目录，比较目标 asset 的 SHA-256，校验 archive 中的 `skilltape` binary，并在最后一步才原子替换目标文件。下载失败、checksum 不匹配、archive 中缺少 binary 或权限失败时，已有 binary 不会被覆盖。版本、下载根地址和 target 都可以固定，脚本不会读取或写入 token、cookie、环境秘密或项目 `.env`。

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

## 本地验证

与 CI 一致的本地门禁：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo run -p skilltape-cli -- lint examples/minimal-skill
```

无效 fixture 应明确失败（当前 policy code 为 3）：

```bash
if cargo run -p skilltape-cli -- lint tests/fixtures/invalid-skill; then
  echo "invalid fixture unexpectedly passed" >&2
  exit 1
fi
```

## GitHub Actions

复制或启用 `.github/workflows/skill-verify.yml` 时，将 `skill_path` 指向仓库内已经审查的 Skill 目录。模板只 checkout 当前仓库、构建本地 CLI、运行 `lint`；它没有 artifact upload、Tape/Receipt 上传、远程 provider 或 secret dump 步骤。若要在 CI 生成 Receipt，请把它留在 runner 临时目录并显式清理，除非仓库另有经过审查的发布策略。
