# Knowledge reference handoff

This source example preserves synthetic content-knowledge material with guarded file
copies and exact SHA-256 assertions. No external process or network is used.
See the [domain guide](../../../docs/guides/domain-workflows.md) for commands,
expected failure behavior, portability, and the distinction from agent review.

Input files: `note.md`, `source.md`.

Changing an input without reviewing its expected hash must fail verification.
Use a temporary receipt path; the CLI refuses to overwrite existing receipts.
