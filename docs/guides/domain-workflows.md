# Domain workflow examples

These source examples prepare small local materials for human review. They reuse
SkillTape's existing file actions, declared permissions, SHA-256 assertions,
Receipts, and exporters. They add no executables, network access, credentials,
model providers, or external platform connections.

| Example | Input | Verified material | Intended next step |
| --- | --- | --- | --- |
| `code-review` | Synthetic `change.diff` | An unchanged patch in `outputs/review/` | A person or separately authorized tool reviews the change |
| `knowledge-reference` | Synthetic note and source Markdown | Both files in `outputs/knowledge/` | Review references before adding the material to a knowledge base |
| `data-export` | Synthetic `metrics.csv` | An unchanged export in `outputs/data/` | Review the data before any analysis or publication |

The asserted hashes describe the included fixtures. A changed or missing input
must fail verification. An artifact hash establishes byte integrity; it does not
establish code correctness, source truth, data quality, or approval to execute a
Skill. Review the input and expected hash together when adapting an example.

## Run one example from the repository root

Use a source build for the current catalog and streaming-hash implementation.
The catalog is not bundled in the published v0.1.0 download assets.

```bash
cargo build --locked -p skilltape-cli
example="examples/domain-workflows/code-review"
receipt_dir="$(mktemp -d)"
target/debug/skilltape lint "$example" --strict --json
target/debug/skilltape verify "$example" --input "$example/fixtures/input" \
  --receipt "$receipt_dir/review.json" --json
target/debug/skilltape export "$example" --target codex \
  --output "$receipt_dir/exported" --json
```

Substitute `knowledge-reference` or `data-export` for the other examples. A
successful Receipt reports `status: succeeded`; modified fixture material reports
`status: run_failed` and exits with code 3. Receipt and export destinations must be new
paths. The CLI does not overwrite existing Receipts.

These packages contain only guarded file operations and assertions. They do not
test an OS process sandbox. Linux/macOS process workflows still require their
existing restricted executors; Windows process Replay/Verify remains fail-closed.
Exporter layouts are project-maintained adaptations, not Agent-vendor
certification or proof of a runtime integration.

## Resource behavior and capacity

Tree digests and workflow file-hash assertions read content in 8 KiB chunks while
preserving the existing digest format. The file inventory still grows with the
number and length of paths. File copies and hashes perform work proportional to
input size; an 8 KiB content buffer does not mean the entire program uses 8 KiB.
The included input files are deliberately small synthetic materials.

Keep normal datasets out of fixtures. Larger real inputs require review of file
counts, storage space, privacy, and run time. No production capacity claim follows
from these examples. No real code review, knowledge-base connection, database,
or analytics provider was used by their checks.

## Next adaptation slices

1. Add domain-specific assertions for bounded, versioned input formats.
2. Add explicit fixture-size and inventory limits with clear failure evidence.
3. Connect separately approved local review/formatting commands while preserving
   executable allowlists and platform sandbox requirements.
4. Review external Agent layout changes against their primary documentation
   before changing exporters.

See [security](../../SECURITY.md), [quickstart](quickstart.md), and the
[adapter contract](../reference/adapter-api.md) for the surrounding boundaries.
