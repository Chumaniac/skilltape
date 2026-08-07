# SkillTape English Documentation Migration Design

> Status: Approved
> Date: 2026-08-07
> Scope: Whole repository

## Goal

Make the entire tracked repository English-only for natural-language content,
organize the documentation into a predictable information architecture, and
make every public description, status statement, link, and validation command
accurate without changing product behavior or protocol contracts.

## Scope and language policy

The migration covers every tracked Markdown file, including public documents,
examples, test fixtures, historical design documents, implementation plans,
and task reports. It also covers natural-language descriptions and user-facing
strings in YAML, JSON, Rust, TypeScript, shell, and workflow files when they
are part of the repository's public or developer-facing surface.

The following values remain exact and are not translated or semantically
rewritten:

- Schema identifiers, enum values, protocol field names, CLI flags, commands,
  paths, URLs, package names, target triples, and environment variable names.
- Code, configuration syntax, test fixture values, hashes, timestamps, and
  historical commit identifiers.
- Product names such as SkillTape, Tape, Console, Replay, Verify, and Receipt.

All explanatory prose, headings, status labels, descriptions, comments,
example narratives, error copy, and documentation metadata use English.
English is the canonical source language; no parallel Chinese translation is
created in this migration.

## Documentation information architecture

The existing paths are retained unless a filename is non-English or clearly
misclassified, which minimizes link churn and preserves historical references.
The canonical structure is:

```text
README.md                         Project entry point and quick start
CONTRIBUTING.md                   Contribution workflow and quality gates
SECURITY.md                       Threat model and vulnerability reporting
CHANGELOG.md                      Versioned release history
docs/README.md                    Documentation index and reading paths
docs/guides/                      Installation, usage, CI, and release guides
docs/reference/                   Stable formats, schemas, and extension APIs
docs/design/                      Product and architecture design records
docs/superpowers/specs/           Approved product and implementation specs
docs/superpowers/plans/           Implementation plans and execution checklists
docs/superpowers/reports/         Historical task and diagnostic reports
examples/                         Runnable example packages
tests/fixtures/                   Intentionally valid or invalid test packages
```

Historical SDD reports are moved to `docs/superpowers/reports/`. The move is
documentation-only: report contents, dates, task identifiers, and evidence
remain intact after translation, and all references are updated.

## Content requirements

### Public entry points

`README.md` must identify the product, show the Capture-to-Receipt workflow,
document platform support, provide a minimal quick start, link to the
documentation index, and state release limitations without stale branch or
pull-request claims.

`CONTRIBUTING.md`, `SECURITY.md`, and `CHANGELOG.md` must use consistent
terminology, English headings, current commands, and accurate release status.

### Guides and reference material

The installation guide is the source of truth for source builds, release
archives, installers, Console assets, platform prerequisites, and local CI
verification. Reference pages must distinguish normative protocol rules from
examples and link to the canonical JSON schemas.

The documentation index must provide an audience-oriented path for users,
Skill authors, adapter authors, contributors, and release maintainers. It must
also identify historical design and execution records as archival material.

### Historical documents

Designs, plans, and reports are translated rather than deleted. Their original
dates, version labels, task boundaries, acceptance criteria, and recorded
limitations are preserved. Current status is clarified where a historical
statement is no longer true, using an explicit archival note rather than
silently rewriting historical evidence.

### Descriptions and UI copy

All package descriptions, example descriptions, workflow labels, installer
messages, Console page descriptions, and test assertions that expose natural
language must be English and use the same product vocabulary as the docs.

## Migration phases

1. Create the documentation index and this approved migration specification.
2. Translate and normalize public entry points, installation, security,
   changelog, and release-readiness documents.
3. Translate and normalize reference pages, examples, and test-fixture docs.
4. Translate design, specification, plan, and report archives; move reports
   to the documented archival path and update links.
5. Translate remaining source/configuration descriptions and user-facing copy;
   update affected tests without changing behavior.
6. Run repository-wide language, link, formatting, build, test, and release
   fixture checks.
7. Commit the documentation migration as a focused change and publish it only
   after the remote-write scope is confirmed.

## Verification and acceptance criteria

The migration is complete when all of the following are true:

- `git grep` finds no Han characters in tracked files.
- Every Markdown file has an English title and English prose.
- Every moved or renamed document has updated inbound links; no stale
  historical report-path documentation links remain.
- Repository, package, workflow, installer, and Console descriptions are
  English and consistent.
- Code blocks, schema identifiers, commands, paths, and protocol examples are
  unchanged except for required documentation corrections.
- Markdown links resolve to tracked files or intentionally external URLs.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passes.
- `CI=1 cargo test --locked --workspace -- --test-threads=1` passes.
- Console build and browser tests pass after any UI copy updates.
- Release package, release workflow, and installer fixture tests pass.
- Release-readiness and changelog statements describe the actual merged main
  branch, CI evidence, and unpublished/published release state.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Translation changes a protocol or security meaning | Preserve normative terms, schema values, commands, and examples; review security and reference pages separately. |
| Historical evidence is accidentally rewritten | Preserve dates, task IDs, evidence, and limitations; add archival context explicitly. |
| A file move breaks links or automation | Use tracked-file link scans and update workflow/test references in the same change. |
| English copy changes UI test behavior | Update only expected natural-language strings and rerun the Console suite. |
| Documentation claims drift from release state | Reconcile README, installation, changelog, and release-readiness against the current GitHub state before commit. |

## Non-goals

- No changes to Rust execution, policy, sandbox, schema validation, or export
  behavior.
- No dependency upgrades or release tag creation.
- No GitHub Release publication or other release-side effects.
- No translation of user-provided fixture payloads when they are intentionally
  testing arbitrary bytes rather than documenting the product.
