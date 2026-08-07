# Tape Format v1

A Tape is SkillTape's immutable, recoverable, auditable record. A Tape is a directory:

```text
tape_<id>/
├── manifest.json
├── events.jsonl
└── .store.lock              # 运行时锁，不是业务事件
```

`manifest.json` uses the fixed schema `skilltape.dev/tape/v1` and contains `id`, `started_at_ms`, `finished_at_ms`, `platform`, the workspace-relative `workspace_root`, and `event_count`. Each line of `events.jsonl` is a JSON `TapeEvent` governed by the `skilltape.dev/tape/v1` event contract.

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

The stable `kind` set is `session_started`, `session_finished`, `terminal_command`, `filesystem_changed`, `permission_requested`, `permission_decided`, `environment_snapshot`, and `capture_warning`; the stable `source` set is `cli`, `shell`, `filesystem`, `permission`, `environment`, and `system`.

**Normative sequence rule.** `sequence` starts at 0 and increases strictly by one, and `event_count` must equal the number of JSONL events.

**Normative locking and recovery rules.** TapeStore locks the store while appending events or finishing a Tape, fsyncs event and manifest updates, repairs the manifest when a single already-written event is detected, and validates consistency. Manual JSONL edits cause reading or replaying the Tape to fail; earlier events are not silently discarded.

## Normative privacy and path rules

- **Normative capture rule.** By default, Capture records only output summaries, sizes, hashes, paths, and redacted metadata; it does not write secret environment variables to a Tape.
- **Normative redaction rule.** `redaction` must accurately express `unredacted`, `redacted`, or `partially_redacted`; downstream UIs must not treat a summary as raw output.
- **Normative path rule.** `workspace_root`, paths in filesystem payloads, and subsequent Workflow paths must be workspace-relative. Absolute paths, Windows drive paths, UNC paths, and `..` traversal are invalid.
- Consumers must apply the schema's unknown-field policy when parsing fields and must not infer new event kinds from natural language; new fields or extensions must use the corresponding `v2` schema.

## Related versioned documents

SkillTape documents use independent stable schemas. See the [reference index](README.md) for the schema-family map and the canonical machine-readable definitions under [`schemas/`](../../schemas/).

| Document | Schema | Purpose |
| --- | --- | --- |
| Tape event | [`skilltape.dev/tape/v1`](../../schemas/tape/v1.json) | immutable recorded event stream |
| Skill package | [`skilltape.dev/skill/v1`](../../schemas/skill/v1.json) | package manifest and entrypoints |
| Workflow | [`skilltape.dev/workflow/v1`](../../schemas/workflow/v1.json) | constrained executable steps |
| Permissions | [`skilltape.dev/permissions/v1`](../../schemas/permissions/v1.json) | filesystem/process/network/secret policy |
| Lockfile | [`skilltape.dev/lock/v1`](../../schemas/lock/v1.json) | engine, tools, and scripts provenance |
| Run event | [`skilltape.dev/run/v1`](../../schemas/run/v1.json) | redacted step timeline |
| Replay summary | [`skilltape.dev/replay/v1`](../../schemas/replay/v1.json) | bounded replay result |
| Receipt | [`skilltape.dev/receipt/v1`](../../schemas/receipt/v1.json) | comparable verification result |

A producer must not reuse a schema version after changing required fields, path rules, enum values, or redaction semantics.
