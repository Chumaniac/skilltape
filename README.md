# SkillTape

SkillTape 是一个 local-first、可回放验证的 Agent Skill 编译器。

它把用户真实完成的一次终端和文件工作流，转换成可审计、可复用、可提交到 GitHub 的 Skill 包。

当前状态：设计阶段，尚未实现运行时代码。

## 核心流程

```text
Capture → Compile → Verify → Share
```

## 设计文档

- [完整设计方案](docs/design/2026-08-04-skilltape-design.md)

## 设计目标

- 本地优先，不强制云端服务
- LLM 生成结构化 Workflow IR，不直接执行任意 Shell
- 默认拒绝未声明的文件、网络和进程权限
- 通过 fixtures、回放和 Receipt 证明 Skill 的执行结果
- 通过 Git、通用文件格式和适配器连接不同 Agent 平台

## MVP 边界

第一版优先支持 macOS/Linux、终端捕获、文件变化捕获、Skill 编译、权限审查、受控回放、本地 Web 查看器和通用 Skill 导出。

