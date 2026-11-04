# Priority fixes verification

Status: source candidate. Date: 2026-11-03, Asia/Shanghai.

This implements the four follow-ups from the [historical source review](https://github.com/Chumaniac/skilltape/blob/12ad9107fe5320134cec274358fba5174c4f4fa4/docs/reviews/2026-11-02-project-review.md).
Existing Tapes, Receipts, release archives and local installations were preserved.

| Area | Behavior and scope | Regression evidence |
| --- | --- | --- |
| Artifact I/O | Shared entry, depth, per-file and total-byte limits; bounded copies and hashes with changing-file/no-link checks | Over-cap traversal/copy fixtures failed before the repair; exact-limit and repeated-hash cases pass afterward. These are source I/O limits, not an OS RSS or disk quota. |
| Execution parameters | Shared nonzero process/stream limits and representable whole-millisecond timeout validation before execution or input reads | Zero, oversized, fractional and unsafe-integer limits reject without creating a new context. |
| Console | Actual Replay schema and explicit legacy aggregate handling; bounded package/documents, 100-row pagination and lazy limited JSON preview | API invalid-document cases, modern/legacy contracts and real-browser fixtures pass without rewriting saved documents. |
| Installer | Stage and back up the CLI, API and UI before replacement; restore all three on failure | Final replacement, fresh install, retained recovery, data preservation and existing-lock cases use isolated synthetic packages. |

The validation receipt records workspace tests, Clippy, formatting, Console build
and browser tests, release-package/workflow fixtures, language and local-link
checks. The Console browser suite contains existing nonfatal duplicate-key and
color-option warnings. No claim of zero browser diagnostics is made.

PowerShell fixtures use mocked downloads and synthetic files. Native Windows
execution is a hosted CI gate; macOS PowerShell checks are separate evidence.
No user installation, paid Provider, production deployment or versioned release
was performed. Cross-platform sandbox support and historical release limits
remain as documented in the [installation guide](../guides/installation.md).
