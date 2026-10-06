# Verified local delivery

Use a source build to preserve a completed workflow's actual output files and
independently inspect them with SkillSync. This feature is available from source;
published SkillTape v0.1.0 binaries do not include `--delivery-dir`.

## Run the synthetic order summary

The [example](../../examples/verified-order-delivery/README.md) runs an existing
system Perl interpreter through a locked shell launcher in SkillTape's restricted
executor. The launcher selects a declared native Perl binary on macOS without
changing the sandbox or inheriting environment variables. It computes daily
order counts and integer-cent totals from three fictional input rows. It uses
no account, model, provider, network, customer database or real order data.

From the SkillTape source checkout:

```bash
cargo build --locked -p skilltape-cli
example="examples/verified-order-delivery"
job_dir="$(mktemp -d)"
target/debug/skilltape lint "$example" --strict --json
target/debug/skilltape verify "$example" --input "$example/fixtures/input" \
  --delivery-dir "$job_dir/delivery" --json
```

`$job_dir/delivery` contains `artifacts/metrics.csv`, `artifacts/summary.json`,
`receipt.json` and `delivery.json`. Stdout remains the existing v1 Receipt.
Review the script, permissions and files before sharing them.

In a matching SkillSync source checkout, build with `npm ci && npm run
build`, then use the same delivery path:

```bash
node dist/cli/index.js artifacts \
  --contract fixtures/product/order-summary/contract.json \
  --delivery /path/to/job/delivery --format json
```

The example produces two daily records, three orders and 3,600 synthetic cents.
The checker reads actual files, checks Receipt/file hashes and CSV/JSON rules,
then independently compares record counts and integer sums. Valid results return
`status: passed` and exit 0. Damage, duplicates, unsafe text prefixes or mismatched
totals return failed findings and exit 1; invalid contracts return exit 2.
Reports contain no cells, JSON values, raw process output or host absolute paths.

## Publication and recovery

`--delivery-dir` conflicts with standalone `--receipt`; the bundle owns its
Receipt. Destinations must be new and outside package/input trees. Existing
destinations, unsafe ancestors and overlaps are rejected. Linux/macOS use native
exclusive rename, including when an empty destination appears after preflight.
Other producer platforms fail closed for exclusive publication.

Failed workflows do not publish a bundle. If assembly/publication fails after a
successful Verify, completed files remain in a private sibling directory named
`.skilltape-delivery-*`. The error gives its generated basename and reason.
Recover those existing materials before considering another execution.

Assembly preserves the completed Receipt before inspecting artifact limits.
Its [versioned manifest schema](../../schemas/delivery/v1.json) describes the
wire fields; relative-path safety, sorting and hash correspondence are also
validated by the producer and consumer.

Assembly streams hashes through 8 KiB and limits artifacts to 16 MiB per file,
64 MiB total, 10,000 entries and depth 64; metadata is limited to 1 MiB. These
bounds cover assembly, not an OS storage quota or a measured RSS ceiling.
An early conservative path/escaping budget bounds manifest and inventory
allocation before serialization. Inventories still grow with entry count and
path length within that budget. Crash/power-loss recovery
is not established by a successful local publication.
Canonical containment and no-follow file reads reject escapes and leaf symlink
replacements. They do not fully isolate against hostile same-user ancestor
directory replacement races.

## Meaning of the evidence

The manifest binds exact Receipt bytes and artifact records. It is an integrity
link, not a signature or authenticated producer identity. The independent
contract checks declared requirements, not input truth or general business
correctness. The current runtime proof uses a macOS restricted process and
synthetic data. Linux requires Bubblewrap; other runtime acceptance is separate.
