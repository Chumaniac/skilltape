# Tape Format v1

Tape 是 SkillTape 的不可变、可恢复、可审查记录。一个 Tape 是一个目录：

```text
tape_<id>/
├── manifest.json
├── events.jsonl
└── .store.lock              # 运行时锁，不是业务事件
```

`manifest.json` 的 schema 固定为 `skilltape.dev/tape/v1`，包含 `id`、`started_at_ms`、`finished_at_ms`、`platform`、workspace-relative 的 `workspace_root` 和 `event_count`。`events.jsonl` 每行一个 JSON `TapeEvent`，schema 同样为 `skilltape.dev/tape/v1` 的事件契约。

## Event fields

```json
{
  "sequence": 0,
  "occurred_at_ms": 1730000000000,
  "kind": "terminal_command",
  "source": "shell",
  "payload": {
    "command": "/bin/echo",
    "stdout_sha256": "<64 lowercase hex characters>",
    "stdout_bytes": 1
  },
  "redaction": "redacted"
}
```

`kind` 的稳定集合为 `session_started`、`session_finished`、`terminal_command`、`filesystem_changed`、`permission_requested`、`permission_decided`、`environment_snapshot` 和 `capture_warning`；`source` 的稳定集合为 `cli`、`shell`、`filesystem`、`permission`、`environment` 和 `system`。

`sequence` 从 0 开始且严格递增，`event_count` 必须与 JSONL 事件数一致。TapeStore 在追加事件和完成 Tape 时执行锁定、fsync、manifest 修复和一致性校验；手工修改 JSONL 会在打开/回放时失败，不会静默丢弃前面的事件。

## Privacy and path rules

- Capture 默认只记录输出摘要、大小、hash、路径和脱敏后的元数据，不把秘密环境变量写入 Tape。
- `redaction` 必须准确表达 `unredacted`、`redacted` 或 `partially_redacted`，下游 UI 不应把摘要当作原文。
- `workspace_root`、filesystem payload 中的路径和后续 Workflow 路径必须是 workspace-relative；绝对路径、Windows drive path、UNC path 和 `..` traversal 都无效。
- Consumer 必须按 schema 字段解析未知字段策略，不应通过自然语言猜测新的事件 kind；新字段或扩展应使用相应的 `v2` schema。

## Related versioned documents

Tape 编译和回放产生的文档也有独立的稳定 schema：

| Document | Schema | Purpose |
| --- | --- | --- |
| Skill package | `skilltape.dev/skill/v1` | package manifest and entrypoints |
| Workflow | `skilltape.dev/workflow/v1` | constrained executable steps |
| Permissions | `skilltape.dev/permissions/v1` | filesystem/process/network/secret policy |
| Lockfile | `skilltape.dev/lock/v1` | engine, tools, and scripts provenance |
| Run event | `skilltape.dev/run/v1` | redacted step timeline |
| Replay summary | `skilltape.dev/replay/v1` | bounded replay result |
| Receipt | `skilltape.dev/receipt/v1` | comparable verification result |

The canonical machine-readable definitions live under [`schemas/`](../../schemas/). A producer must not reuse a schema version after changing required fields, path rules, enum values, or redaction semantics.
