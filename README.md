<img src="apps/skilltape-console/src/assets/skilltape.svg" width="56" height="56" alt="" />

# SkillTape

[![Build](https://github.com/Chumaniac/skilltape/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Chumaniac/skilltape/actions/workflows/ci.yml) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-555)](LICENSE)

记录一次本地命令，把输入、执行步骤和验证凭据放进同一份可复查的交付包。

[在线核对数量](https://chumaniac.github.io/skilltape/) · [CLI 下载](https://github.com/Chumaniac/skilltape/releases/tag/v0.4.1) · [中文桌面包（Apple Silicon）](https://github.com/Chumaniac/skilltape/releases/tag/user-2026.11.11-zh)


在线清单页的“第一次使用”提供示例、计算和导出入口；本机控制台另有证据包、时间线与复核引导。清单核对用于熟悉输入输出，完整流程录制与 Receipt 使用 CLI。

## 从一份对账单开始

网页版可编辑应有与实收数量、查看差异、导出 JSON。计算在当前标签页完成。要重放命令或生成 Receipt，请使用本机 CLI。

1. 在网页版保存输入 JSON。
2. 从本仓库构建 CLI。Linux 需安装 bubblewrap，macOS 需 Xcode Command Line Tools。Rust 版本见 [工具链配置](rust-toolchain.toml)。
3. 把下面的 `request.json` 换成导出的文件。两个输出目录应当尚未存在。

```sh
git clone https://github.com/Chumaniac/skilltape.git
cd skilltape
cargo build --locked -p skilltape-cli
./target/debug/skilltape starter new reconcile --request request.json --output inputs
./target/debug/skilltape starter run inputs --output run --yes
```

## 可以做什么

- **记录与重放**。采集本地命令和输入，编译成 Skill，在系统沙箱中验证。
- **复核结果**。在控制台查看 Receipt、失败检查与输入快照。批次支持逐项重试并保留早期结果。
- **交付**。导出输入、结果和校验记录。可用 SHA-256 检查输出是否逐字节匹配。

用「仅看差异」筛选后，「导出当前表格 CSV」只包含屏幕上的行。CSV 可在 Excel 中继续筛选，完整结果与任务输入仍可分别导出 JSON。

## 常见问题

**需要配置账号或环境变量吗？** 本地入门流程无需密钥或环境变量。构建控制台使用 Node 24，桌面运行时、备份和恢复见[构建说明](docs/build.md)与[使用指南](docs/user-guide.md)。

**Windows 能重放吗？** Windows 可以查看记录和准备输入。Replay 和 Verify 需要受支持的系统沙箱。

**下载版和主分支有哪些差异？** v0.4.1 是固定版本。上面的 starter 导入流程使用主分支源码。中文桌面包保留原始发布版本及校验文件。

[安装 CLI](docs/guides/installation.md) · [批次验证](docs/verification-batches.md) · [贡献指南](CONTRIBUTING.md) · [报告安全问题](SECURITY.md)

**Beta** · Turn a real local workflow into a reviewable Agent Skill you can replay and verify before you share it.

[终端示意](docs/assets/quickstart-terminal.svg) · [纯文本操作记录](docs/assets/quickstart-terminal.txt) · [命令行入门](docs/guides/quickstart.md) · [Configuration](docs/guides/configuration.md)
