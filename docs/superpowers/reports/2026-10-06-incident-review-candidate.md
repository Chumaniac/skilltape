# Incident review material candidate

Status: Local candidate; not pushed, submitted, merged, or released.
Date: 2026-10-06.
Source base: `4d0a22b8091814480e8e14fa51252bee0c9968a6`.

## Scope and acceptance

The `incident-review` package prepares a fictional incident report and review
runbook with guarded copies and exact SHA-256 assertions. The two included files
total 903 bytes. Four native steps copy and check the materials; no executable,
network, model provider, or environment access is declared. The output contains
unchanged review documents and a Receipt, not a root-cause analysis or recovery.

- Security: exact read/write scopes, an undeclared write denied before copying,
  and digest-only Receipt evidence. No real logs or credentials were used.
- Function: valid material succeeds; modified or missing material fails with
  `run_failed` and exit 3. Existing exporter layouts remain usable.
- Resources: two small synthetic inputs and no added dependency or process.
  This fixture is bounded; the engine still has no general input-inventory limit.
  CPU, RSS, large datasets, live incidents, and Windows execution were not measured.

## Local verification

- Three domain CLI tests passed, exercising four domains, 16 exports, four
  successful Receipts, four changed-input failures, four missing-input failures,
  and one undeclared-write denial.
- Existing path and output-redaction security tests: seven passed.
- CLI-target Clippy with warnings denied and workspace formatting passed.
- Documentation contracts: ten passed. Repository-wide English scan had no CJK
  matches. The rendered Markdown link audit validated actual links with positive
  and negative controls; no target was missing.
- The guide's simple link regex flags eight pre-existing paths in fenced examples
  in the unchanged historical user-documentation plan. They are not rendered links.
  The first parser audit had an incompatible legacy Parser object and was invalid;
  it was corrected and rerun with controls rather than counted as a pass.
- Matching blog overview candidate: 22 content checks, 136-file typecheck with
  zero issues, lint/format, a 35-route build/resource budget, and Chromium,
  Firefox, and WebKit checks at 1440/320/390 passed. The new row and local-candidate
  notice were checked, with a mobile visual review.
- The first local build was blocked at the TSX IPC setup. The authorized local
  rerun passed. This was a setup restriction, not an application reproduction.

Full workspace/Console suites, candidate remote CI and CodeQL, a release, any live
Agent/service integration, and production deployment were not performed.
The currently published v0.1.0 assets remain separate from this local candidate.

See the [domain guide](../../guides/domain-workflows.md) for commands and boundaries.
The next rotation is SkillSync: inspect existing preflight resource limits before
adding a bounded material-capacity check with synthetic negative cases.
