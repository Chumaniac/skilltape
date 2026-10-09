# Stable export and optional Receipt association

Status: source candidate 0.2.2, 2026-10-09. These additions are not in the
existing public v0.2.1 assets. Ordinary Export remains a lint-only packaging
operation; it does not run a Skill or authenticate its author.

## Stable selected-file export

The exporter uses its existing required/optional/script allowlist. It enumerates
at most 10,000 entries, including directories, with depth 64, a 16 MiB file limit
and 64 MiB selected-byte total. Referenced metadata documents are also read with
a 16 MiB ceiling before parsing. Whole files are not allocated just to hash or
copy them: transfer uses an 8 KiB buffer and an observed-size read plus one probe.

Detected replacements, growth, truncation, symlinks, unsupported file types or
changes in the selected inventory stop publication. The copied package is
loaded and linted, and the manifest's `package_hash` is calculated from the actual
staging bytes. The sorted relative paths, zero separator, big-endian u64 size and
contents keep the existing deterministic digest algorithm.

Native no-replace directory publication is shared across Generic, Codex,
Claude Code and Cursor. Linux/macOS use exclusive rename flags. Windows uses
[MoveFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefilew),
which rejects an existing destination and directory moves across volumes.
This filesystem support does not enable Windows Replay or Verify.

These are checks of a caller-selected snapshot. Metadata checks do not provide
an OS sandbox against a same-privilege process actively replacing directory
parents. A package must still be reviewed before anyone executes it.

## Associate a successful Receipt

Build the current source first. On a platform with the existing restricted
executor, use an independent new workspace and retain the Receipt outside the
package tree:

```bash
cargo build --locked -p skilltape-cli
skilltape="target/debug/skilltape"
workspace="$(mktemp -d)"
"$skilltape" capture demo --workspace "$workspace" --command /bin/echo --output "$workspace/tape" --yes --json
"$skilltape" compile "$workspace/tape" --output "$workspace/skill"
"$skilltape" verify "$workspace/skill" --receipt "$workspace/receipt.json" --json
"$skilltape" export "$workspace/skill" --target generic \
  --output "$workspace/export" --receipt "$workspace/receipt.json" --json
```

The optional guard rejects Receipt files over 1 MiB, ambiguous duplicate JSON
properties, wrong schemas, failed or internally inconsistent status and unmatched
step identities. The actual staged package digest must equal `skill_hash`.
Unrepresented files in a verified source tree therefore prevent association;
clean and re-verify the intended package instead of weakening the guard.
Invalid or mismatched evidence produces no published destination. Existing
destinations are never replaced. Normal Export without the flag stays available.

The JSON manifest adds only a bounded `receipt` reference: run/Skill identity,
the exact Receipt SHA-256 and `provenance: not-authenticated`. Raw Receipt,
command output and credentials are not added by this flag. A forged successful
Receipt can have internally consistent declarations; the guard matches metadata
and bytes, not a signature, trusted Provider or fresh execution in another host.

See [verified deliveries](verified-delivery.md) for retained output materials and
the [expansion roadmap](../deep-optimization-roadmap.md) for the separate future
authentication and runtime work.
