# Verified multi-organization delivery

Status: source catalog example, 2026-10-10. SkillTape binary behavior remains
source0.2.2; the composite consumer needs SkillSync source0.1.6. Existing public
SkillTape0.2.1, SkillSync GitHub0.1.4 and npm0.1.0 assets are unchanged.

Two fictional organizations both use ORDER_1. The locked local script reads
orders and split shipments, writes stable expected.csv/allocations.csv and a
summary.json, and SkillTape preserves actual files with a Receipt and manifest.
The included consumer contract compares the ordered tenant/order key and integer
totals using actual delivery bytes. No account, provider, network, database or
payment is connected; source truth and authorization are not authenticated.

The producer accepts only its small simple unquoted CSV format and ASCII identity
alphabet. Both files together are limited to1MiB and1,000 data rows, each line
to256 characters. Nonnegative safe integer minor units and per-table totals are
required. Orders require a unique tenant/order pair; shipments may split a pair.
Output groups use nested maps, stable ordering and exclusive file creation.
The process has the existing three-second timeout and one-process policy.

Build both repositories from source, then use separate new job destinations:

```bash
cargo build --locked -p skilltape-cli
example="examples/verified-tenant-delivery"
workspace="$(mktemp -d)"
target/debug/skilltape lint "$example" --strict --json
target/debug/skilltape verify "$example" --input "$example/fixtures/input" \
  --delivery-dir "$workspace/good" --json
node /path/to/skillsync/dist/cli/index.js artifacts \
  --contract "$example/fixtures/consumer-contract.json" \
  --delivery "$workspace/good" --format json
```

The producer succeeds and the composite consumer exits0. Preserve this success;
create a fault copy with unchanged totals and counts:

```bash
cp -R "$example/fixtures/input" "$workspace/fault-input"
cp "$example/fixtures/faults/shifted-shipments.csv" \
  "$workspace/fault-input/shipments.csv"
target/debug/skilltape verify "$example" --input "$workspace/fault-input" \
  --delivery-dir "$workspace/fault" --json
node /path/to/skillsync/dist/cli/index.js artifacts \
  --contract "$example/fixtures/consumer-contract.json" \
  --delivery "$workspace/fault" --format json
```

SkillTape still succeeds: it faithfully executes the producer and retains the
wrongly allocated inputs' output. SkillSync exits1 with two tuple mismatches,
while a rule using only order_id can accept the same global3,000 total. Consumer
reports retain execution:not-run (no Skill was executed by that consumer) and
provenance:not-authenticated. The separately recorded SkillTape execution is
actual local synthetic process evidence, not customer or platform acceptance.

Linux/macOS require the existing restricted executor, /bin/bash and a declared
native system Perl. Windows process Replay/Verify remains unsupported. The native
Bash launcher avoids macOS sh-selection diagnostics under restricted file access.
Interpreter selection never changes the sandbox or falls back to unrestricted execution.
Invalid input produces one fixed failure without printing identifiers or values;
no completed delivery is published. Existing outputs are never replaced; a rare
filesystem write failure may leave partial files inside disposable staging.
They are not a successful delivery or an invitation to rerun successful assets.
