# Execution policy preflight

Run `skilltape preflight examples/minimal-skill --json` before replay or verification. The command loads and lints the package, then uses the same pure capability checks as replay. It does not open input data, create an execution workspace or output, spawn commands, contact services, or create a Receipt.

The versioned `skilltape.dev/policy-plan/v1` document contains `execution: "not-run"`, the declared-policy result, package validity and finding codes, and ordered per-step policy decisions. It includes no command arguments, environment values or process output. Exit codes are 0 for an allowed package, 3 for a blocked package, and 2 for an unreadable/invalid package or a preview exceeding 1,000 steps/checks. Output can be redirected by the caller; the tool itself does not write a plan file.

The Console's Permissions page shows the same preflight from its existing read-only package response. It renders at most 32 checks; the CLI provides the full bounded plan. Older APIs and packages over the preview limit show an unavailable state rather than an approval.

A successful preflight is limited to the declared contract. It does not establish that input data is valid, a platform sandbox is available, an executable will succeed, or output requirements will pass. Run the existing verification path for a real Receipt.

Method references from the 2026-10-19 Trending review: [OpenShell's layered capability boundaries](https://github.com/NVIDIA/OpenShell/blob/0bfa1e8b0261ca34aee134dda2829aa15f2a7b90/README.md) (Apache-2.0), and [CLI-Anything's structured, verifiable CLI output](https://github.com/HKUDS/CLI-Anything/blob/34f519533bc175d2fe287ab8316b0dd99bb9cc43/README.md) (Apache-2.0). No upstream runtime or plugin is installed.
