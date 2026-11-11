# Verify exact output bytes

Status: included in the v0.3.0 distribution; absent from v0.2.3 archives.
Date: 2026-10-18.

An existing file is insufficient evidence that a workflow produced the expected
bytes. Supply a known digest from an independently reviewed reference:

```sh
skilltape verify ./skill --input ./input \
  --expect-file summary.txt \
  --expect-sha256 result.txt=5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03 \
  --delivery-dir ./delivery --json
```

The example digest describes `hello` followed by a newline. Use the digest of
your own reviewed reference; hashing the just-produced result as its own
expectation would not provide an independent check.

Each output path is relative to the output root; the existing `outputs/` prefix
is also accepted. Repeat the option for several outputs. Hex digits are
case-insensitive and normalized before binding the requirements to the run ID.
Repeated equivalent digest requirements are checked once. Different digests
for the same normalized path are rejected before execution. At most 64 combined
`--expect-file` and `--expect-sha256` arguments are accepted, with paths of at
most 1024 bytes. Unsafe paths and malformed requirements return exit 2.

Verify hashes stable regular files in small chunks, with a 16 MiB per-file and
64 MiB combined hash-read budget. Detected file replacement, mutation or budget
exhaustion returns an input error rather than successful evidence. These checks
do not claim protection against every hostile concurrent filesystem mutation.
Missing or mismatched output returns an `assertion_failed` Receipt and exit 3;
the delivery directory remains unpublished. Receipts contain the assertion
target and outcome, not output file contents. No network access or additional
permissions are enabled.

The interface follows the composable command approach in
[CLI-Anything](https://github.com/HKUDS/CLI-Anything) and the independent
evidence distinction in
[Cloudflare's audit skill](https://github.com/cloudflare/security-audit-skill).
Those projects were inspected on 2026-10-18; no upstream code, plugins or
runtime dependencies were added. A matching digest proves byte equality with
the supplied reference, not source authenticity or business approval.
