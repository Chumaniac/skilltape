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
| `incident-review` | Synthetic incident report and review runbook | Both unchanged documents in `outputs/incident/` | A person reviews assumptions before any separately approved operational action |

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

Substitute `knowledge-reference`, `data-export`, or `incident-review` for the other examples. A
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
The included input files are deliberately small synthetic materials. The incident
example contains two documents and no executable commands. Missing material,
modified bytes, or an undeclared write must fail; none of these checks establishes
an incident root cause or authorizes service recovery.

Keep normal datasets out of fixtures. Larger real inputs require review of file
counts, storage space, privacy, and run time. No production capacity claim follows
from these examples. No real code review, knowledge-base connection, database,
or analytics provider was used by their checks.

## Bounded input preflight and I/O in source

The source implementation adds a metadata-only preflight before Verify reads
hash content and before Replay copies inputs. Use a current source build; this
extension is not included in the published v0.1.0 download assets.

| Observed input dimension | Inclusive ceiling |
| --- | --- |
| Descendant entries, including empty directories | 10,000 |
| Descendant depth, with the input root at depth 0 | 64 |
| Logical bytes per regular file | 16 MiB |
| Logical bytes summed over all file paths | 64 MiB |

Oversized static input returns a typed `InputCapacity` error and CLI exit 2 before
run events, output materialization, or a successful Receipt. Nested symlinks and
unsupported filesystem entries are rejected using the existing path checks;
their existing error classification is retained. The scanner streams directory
entries rather than retaining the entire listing, reads no file content, and
adds no dependency or provider access. Code-review patches, knowledge materials,
data exports and incident documents share the same preflight.

Input hashing and staging now build their own bounded inventories using these
same ceilings. Each file read stops at its observed size plus one probe byte;
the probe is never hashed or copied. A growing or truncated file fails instead
of being read to an unbounded EOF. Inventories include directories and are
checked again after I/O, rejecting additions, removals and observed metadata
changes. Regular files are opened no-follow and nonblocking on Unix; Windows
opens reparse points themselves and rejects them. Stable inputs preserve the
existing path/length/content digest, Receipt fields, file contents and modes.

At most 64 MiB of content is hashed or staged per invocation, with at most one
additional probe byte per file. The 8 KiB content buffer is reused; inventories
remain proportional to the bounded entry count and path lengths. Input staging
stays in the existing disposable workspace; failures precede run events and
final output publication. Package scripts and generated outputs retain their
existing separate copy behavior.

These are bounded application reads and staging, **not an operating-system disk
quota or a frozen filesystem**. Hashing and Replay staging check separate source
snapshots; this slice does not authenticate identity or bind a same-user hostile
writer across both phases. Ancestor replacement races remain outside complete
isolation. Package scripts, outputs, execution time and measured RSS are outside
this input budget. Split and review larger datasets deliberately; no automatic
override is available.

Use a direct relative path or an absolute input path without `..` components.
Metadata paths reject parent-directory references before access; the scanner
anchors recursive directory reads to the selected canonical input root.

Local checks use sparse synthetic files, empty directories and the existing small
domain fixtures. They verify rejection and boundaries, not production capacity,
real semantic review or a platform sandbox integration.

## Next adaptation slices

1. Add domain-specific assertions for bounded, versioned input formats.
2. Bind verification and execution to one immutable prepared input snapshot,
   while preserving the existing Receipt and output publication contracts.
3. Connect separately approved local review/formatting commands while preserving
   executable allowlists and platform sandbox requirements.
4. Review external Agent layout changes against their primary documentation
   before changing exporters.

See [security](../../SECURITY.md), [quickstart](quickstart.md), and the
[adapter contract](../reference/adapter-api.md) for the surrounding boundaries.
