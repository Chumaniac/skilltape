# SkillTape · 独立用户版

把本地任务的输入、执行过程、结果与验证凭据放在一起。

保留输入快照、结果哈希和原始 Receipt。复现依赖明确的沙箱边界；检查页面不会执行任意任务。

这是包含完整可构建源码的社区用户仓库，不依赖原私有仓库。旧仓库的提交历史、用户数据、模型缓存和内部验收记录不在这里。

## 开始使用

前提：Node 24、Rust（版本见 rust-toolchain.toml）；macOS 需 Xcode Command Line Tools，Linux 需 bubblewrap。

```sh
git clone https://github.com/Chumaniac/skilltape-app.git
cd skilltape-app
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
cargo build --locked --release -p skilltape-cli -p skilltape-console-api
./target/release/skilltape console --workspace /absolute/path/to/run --open
```

浏览器只连接本机回环地址。桌面版下载与系统支持以 [Releases](https://github.com/Chumaniac/skilltape-app/releases) 的实际资产为准；没有对应资产时从源码安装。桌面构建方式见 [构建说明](docs/build.md)。

## 用一次完整流程

新建输入 → 检查差异 → 导出输入 → 执行固定任务 → 核验并打开结果

1. 打开「本地任务」，用示例填写对账数据，核对差异并导出输入 JSON。页面计算与持久化结果是本地操作，不代表已经获得执行凭据。
2. 用下方命令从导出的 JSON 创建任务材料。检查输入，再显式加 --yes 执行。输出目录必须是全新目录，不能覆盖旧结果。
3. 在 Console 中选择运行目录，查看 Capture、Compile、Lint、Verify、Export 结果以及原始 Receipt。

```sh
./target/release/skilltape starter new reconcile --request /path/request.json --output /path/new-inputs
./target/release/skilltape starter run /path/new-inputs --output /path/new-run --yes
```

## 保存与边界

桌面默认 ~/Library/Application Support/SkillTape/workspace；通过「选择工作区」打开已有运行目录。CLI 写入你指定的新输出目录。

Windows 支持查看和输入准备，Replay/Verify 不支持时会明确拒绝。macOS/Linux 的执行需要系统沙箱；没有沙箱时不会降级成无隔离执行。Receipt 证明特定输入与结果的验证记录，不承诺任务具有任意业务正确性。

- [完整使用说明](docs/user-guide.md)
- [隐私、备份与恢复](docs/privacy.md)
- [排错与环境检查](docs/troubleshooting.md)
- [已验证范围](docs/verification.md)
- [参考项目与许可](docs/references.md)

许可证见 [LICENSE](LICENSE)，第三方资料见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
