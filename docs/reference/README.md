# Reference

This section contains stable, implementation-facing descriptions of SkillTape
formats and extension APIs. The reference pages explain the contracts; the
machine-readable schemas remain the canonical definitions.

- [Tape format](tape-format.md) — recorded events, persistence rules, and recovery behavior.
- [Adapter and plugin API](adapter-api.md) — exporter and plugin interfaces, capabilities, and security requirements.

## Schema families

The [`schemas/` directory](../../schemas/) contains the versioned JSON schema
families. The current repository defines these `v1` families:

- [Lock](../../schemas/lock/v1.json)
- [Permissions](../../schemas/permissions/v1.json)
- [Receipt](../../schemas/receipt/v1.json)
- [Replay](../../schemas/replay/v1.json)
- [Run](../../schemas/run/v1.json)
- [Skill](../../schemas/skill/v1.json)
- [Tape](../../schemas/tape/v1.json)
- [Workflow](../../schemas/workflow/v1.json)

This index identifies the schema families without duplicating their
definitions.
