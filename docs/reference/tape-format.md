# Tape Format v1

A Tape is SkillTape's immutable, recoverable, auditable record. A Tape is a directory:

```text
tape_<id>/
├── manifest.json
├── events.jsonl
└── .store.lock              # runtime lock, not a business event
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


## Compile eligibility in current source



Capture retains a finalized Tape after interruption or a recording error so it
can still be inspected. Current source Compile rejects a manifest that has not
finished and any recorded `session_finished` cancellation, timeout or non-null
recording error. Malformed optional completion flags are input errors. No Skill
is published and the Tape is not changed. Keep the interrupted record and run a
new Capture into a new directory when a complete recording is needed.

Finished legacy metadata-only Tapes remain supported. A completed command's
nonzero exit status is distinct from a Capture interruption; Compile still
produces a reviewable draft, not proof that replay or business checks passed.
The shared compiler input constructor and deserializer enforce explicit
completion evidence, including for direct SDK callers. Tape v1 and existing
v0.4.1 download assets are unchanged; this is a source-build improvement.

## Retained Capture identity

New CLI compilations add an optional `source_tape` to `skilltape.dev/compile/v1`:
`format: skilltape.dev/tape-source/v1`, `manifest_sha256`, `events_sha256`, and
`event_count`. The digests cover compact JSON of the parsed v1 manifest and
ordered event array, with object keys sorted. They identify the values consumed
by Compile, including recorded identity, rather than file formatting, unknown
fields or unrelated Tape files. The summary contains no original workspace
path, recorded identifier or event payload. Repeating the same compilation is
deterministic; different Capture identities remain distinguishable even when
they produce the same executable package.

Compile rechecks manifest values after the bounded event read and stops before
publishing if metadata changed. Tape manifests now have a shared 64 KiB read
limit; new writes also enforce it. Existing event limits remain 16 MiB and
100,000 events. Oversized or interrupted inputs remain on disk for inspection.

Console's read-only Compile review displays the two source digests and event
count. Old packages without this optional metadata explicitly say their source
was not recorded. Malformed claimed source metadata is rejected. These hashes
are provenance metadata, not author authentication or proof of successful
replay. Existing Skill/Receipt hash semantics and exports are unchanged; this
does not retroactively attach source evidence to an old Receipt.

### Commands with concurrent file observations

Filesystem observations may occur between a terminal command's `started` and
`output` events. Compile keeps the command's workflow position at its start and
places its observed-output assertions after it, while retaining original event
sequences. A later command begins a new observation scope so its file changes
cannot replace an earlier command's assertion. Missing outputs, nested starts,
or unpaired output events are still rejected. Capture order, paths, hashes and
replay policy are not rewritten to make a draft pass.

### Data file boundaries

Manifest and event reads resolve inside the selected canonical Tape directory
and require regular files. Symlink and special-file inputs are rejected before
parsing; file opens use no-follow flags and Unix nonblocking mode, followed by
path/handle checks. Unix compares device/inode identity; other platforms retain
regular-file, reparse and metadata checks. Appending events uses the same file
boundary. A FIFO cannot wait indefinitely for a writer. These checks follow the
[CodeQL path validation guidance](https://codeql.github.com/codeql-query-help/rust/rust-path-injection/)
and preserve the selected directory as the authority; they are not a general
operating-system sandbox or author authentication.
