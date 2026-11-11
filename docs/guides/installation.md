# 安装 SkillTape 独立用户版

普通用户请从[本仓库 Releases](https://github.com/Chumaniac/skilltape-app/releases)下载 `skilltape-app-macos-arm64-2026.11.11.zip`、`SHA256SUMS` 与 `USER_GUIDE.md`。当前桌面包面向 Apple Silicon、macOS 13.0 及以上；未提供 Intel、Linux 或 Windows 桌面安装包。

```sh
shasum -a 256 -c SHA256SUMS
```

解压后把 `SkillTape.app` 放入 `~/Applications`，双击打开。包内有 Rust CLI、只读 Console API 与前端，无需源码、Rust 或 Node。程序内“帮助 → 使用指南”可离线查看命令及数据位置。使用本机临时签名，未做 Apple 公证。

更新前保存未完成输入并退出旧程序。保留旧包和私人工作区后，用新包替换程序；不要删除 `~/Library/Application Support/SkillTape/workspace`。运行记录和 Receipt 应另外备份。

## 从源码构建

见[构建说明](../build.md)。源码 CLI 的 macOS/Linux 执行需要各自的系统沙箱：macOS 使用 `/usr/bin/sandbox-exec`，Linux 使用 Bubblewrap 和可用的用户命名空间。没有沙箱时 Replay/Verify 明确拒绝。Windows 的 Replay/Verify 未提供等价隔离执行器，不会降级执行。

此社区用户版不使用旧仓库 v0.4.1 的固定安装器或旧跨平台下载地址。仓库内保留的历史版本说明不等于本版已有相应发行资产。首次完整操作见[用户指南](../user-guide.md)。
