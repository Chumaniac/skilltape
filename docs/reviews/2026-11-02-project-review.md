# skilltape 项目完成情况与优化清单

审查日期：2026-11-02（北京时间）。

审查基线：[`69d1828e9a18a6b50fe25405fd2060036331f238`](https://github.com/Chumaniac/skilltape/commit/69d1828e9a18a6b50fe25405fd2060036331f238)。

本地捕获、编译、核验、回执和导出工具较完整；当前源码功能多于 v0.4.1 下载包。

本次提交新增审查文档。清单中的优化仍为待执行事项，源码判断、历史验证、当前云端结果和实际验收分别记录。

## 已有能力

- Capture→Compile→Lint→Verify→Receipt→Export、策略预检、Console、Starter 和可恢复 Batch 已有代码。 依据：[README.md:14](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/README.md#L14)；[crates/skilltape-cli/src/batch_command.rs:730](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-cli/src/batch_command.rs#L730)。
- Batch 的 prepare 不执行、pending 执行、显式 retry、取消和 unconfirmed 分流已有状态约束。 依据：[crates/skilltape-core/src/batch.rs:182](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-core/src/batch.rs#L182)。
- 输入文件、进程 stdout/stderr、Console 页数、Receipt/context 和 Batch 保留材料已设置多类容量边界。 依据：[crates/skilltape-runner/src/workspace.rs:11](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-runner/src/workspace.rs#L11)；[crates/skilltape-runner/src/lib.rs:65](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-runner/src/lib.rs#L65)；[crates/skilltape-cli/src/batch_command.rs:22](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-cli/src/batch_command.rs#L22)。

## 当前缺口与完成边界

- Starter/Batch 属于 source-main，未随原 v0.4.1 下载包发布；Windows Replay/Verify 继续 fail-closed。 依据：[README.md:14](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/README.md#L14)；[README.md:90](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/README.md#L90)。
- 回执证明受限本地流程和文件关系，不证明认证执行身份、业务语义、远程交付或长期 CPU/RSS。 依据：[docs/release-readiness.md:114](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/docs/release-readiness.md#L114)。

## 优化顺序与验收条件

P1 表示影响正确性、可恢复性或下一阶段交付的优先事项；P2 为可靠性和维护改进；P3 为后续扩展或基线测量。优先级不表示已确认安全漏洞。

### P1 给 Replay 生成文件和脚本复制建立统一预算

证据级别：源码容量缺口，未运行超限复现。

输入预算与进程输出限制不能约束所有生成文件；file_hash、物化和引用脚本复制缺少对称的文件数/字节预算。

验收条件：hash、materialize、delivery manifest 共用单文件/总字节/数量/深度预算；增长、替换或超限时拒绝发布部分交付。

依据：[crates/skilltape-runner/src/execution.rs:212](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-runner/src/execution.rs#L212)；[crates/skilltape-runner/src/workspace.rs:85](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-runner/src/workspace.rs#L85)；[schemas/workflow/v1.json:39](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/schemas/workflow/v1.json#L39)。

### P2 对齐资源配置与 run-context 的有效性约束

证据级别：源码状态约束不一致。

runner validate_limits 未拒绝 max_output_bytes=0，而 context 绑定要求其大于 0，库调用可能产生无法绑定的回执侧车。

验收条件：入口拒绝无效值与超实现上限，验证运行、保存、Console 与 Batch 读取的一致性。

依据：[crates/skilltape-runner/src/lib.rs:463](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-runner/src/lib.rs#L463)；[crates/skilltape-verify/src/context.rs:91](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/crates/skilltape-verify/src/context.rs#L91)。

### P2 给 Console diff/run 输入补齐有界契约校验

证据级别：源码维护与资源风险。

diff 数组只检查结构、run.json 只解析 JSON；大集合渲染或合法 JSON 的错误 schema 可流向客户端。

验收条件：服务端校验 run v1，客户端校验数量/长度并分页；malformed/oversized fixture 不进入页面渲染。

依据：[apps/skilltape-console/src/api.ts:158](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/apps/skilltape-console/src/api.ts#L158)；[apps/skilltape-console-api/src/read_model.rs:504](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/apps/skilltape-console-api/src/read_model.rs#L504)；[apps/skilltape-console/src/pages/CompileReview.tsx:98](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/apps/skilltape-console/src/pages/CompileReview.tsx#L98)。

### P2 让安装器三件套共同提交或回滚

证据级别：源码失败路径风险，未模拟安装失败。

CLI/API 已替换后若 Console 替换失败，当前路径可能仅恢复 Console，留下混合版本。

验收条件：保留旧 CLI/API/UI 并统一事务替换；模拟最后一步失败，确认三件套版本和用户回执/数据均保持。

依据：[scripts/install.sh:166](https://github.com/Chumaniac/skilltape/blob/69d1828e9a18a6b50fe25405fd2060036331f238/scripts/install.sh#L166)。

## 验证记录

- 审查基线 main 的 9 项托管检查均 success，含平台编译/Console/Rust/已有 CodeQL。
- 本轮用户文档检查 10 项通过；没有新增启动扫描或发行操作。
- 未运行 Rust 全套、安装器故障注入、Windows Replay、真实执行任务或新版本发布。

审查基线云端检查摘要：success 9。
- [Analyze (actions)](https://github.com/Chumaniac/skilltape/actions/runs/41275817867/job/124757667248)：success。
- [Analyze (python)](https://github.com/Chumaniac/skilltape/actions/runs/41275817867/job/124757667227)：success。
- [Analyze (javascript-typescript)](https://github.com/Chumaniac/skilltape/actions/runs/41275817867/job/124757667215)：success。
- [Analyze (rust)](https://github.com/Chumaniac/skilltape/actions/runs/41275817867/job/124757667089)：success。
- [Windows production compile](https://github.com/Chumaniac/skilltape/actions/runs/41275818042/job/124757665722)：success。
- [Console (ubuntu-latest)](https://github.com/Chumaniac/skilltape/actions/runs/41275818042/job/124757665710)：success。
- [Rust (macos-14)](https://github.com/Chumaniac/skilltape/actions/runs/41275818042/job/124757665694)：success。
- [Console (macos-latest)](https://github.com/Chumaniac/skilltape/actions/runs/41275818042/job/124757665684)：success。
- [Rust (ubuntu-latest)](https://github.com/Chumaniac/skilltape/actions/runs/41275818042/job/124757665660)：success。
