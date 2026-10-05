# Domain workflow and tree-hash expansion

## Scope

Source base: `9e47e30`. This change adds three small, synthetic material-handoff
examples and streams content hashing through an 8 KiB buffer. It preserves the
v1 tree-digest framing, global path order, Receipt schema, permissions, and
exporter contracts. It does not introduce network/model/provider execution.

## Measured memory boundary

Same macOS host, debug CLI builds, four 16 MiB synthetic input files, and the same
empty workflow. The empty workflow isolates input hashing; it is not a business
workflow or process-sandbox acceptance test. `/usr/bin/time -l` reports:

| Measurement | Before | After |
| --- | ---: | ---: |
| Maximum resident set size | 86,310,912 bytes | 19,070,976 bytes |
| Input size | 67,108,864 bytes | 67,108,864 bytes |
| Input files | 4 | 4 |
| Receipt JSON | Identical | Identical |

The run used only locally generated data. These results do not establish CPU
improvement, a general memory ceiling, or production capacity. Inventories still
scale with path count/length, and execution/copy overhead remains.

## Regression evidence

- Runner and verification tests: 28 passed, including a legacy digest golden
  with binary content, ordering-sensitive paths, and a replaced-parent symlink.
- Domain packages: strict lint; exports to generic, Claude Code, Codex, and
  Cursor layouts; successful fixture Receipts; changed input rejected with
  `run_failed` and exit 3 without raw contents in the JSON summary.
- Included examples: code-review material, a knowledge note plus reference,
  and a synthetic CSV export. All use native guarded file actions and hashes;
  no processes or live Agent integrations are part of these checks.
- Formatting and documentation contracts are checked before submission.
  Remote CI, merge, release, and deployment must be recorded separately.

The public v0.1.0 release assets have not been replaced. The examples and
streaming implementation are source changes until a separate release occurs.
