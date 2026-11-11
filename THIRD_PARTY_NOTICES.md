# 第三方资料与运行时

本仓库自身代码按根目录 LICENSE 授权；第三方资料保留各自许可，不能由根许可证覆盖。

## 独立用户版的运行包

源代码仓库本身不包含依赖可执行文件或用户数据。构建器在组装实际运行包时附带相关许可：

- Rust 依赖由 Cargo.lock 固定；对应许可证和依赖清单随 runtime-licenses/cargo 保留。根 LICENSE 不替代这些许可证。

模型、数据集、Intiface 应用不在运行包中。借鉴来源和未复制的实现见 docs/references.md。
