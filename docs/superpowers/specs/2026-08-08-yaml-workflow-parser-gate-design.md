# YAML workflow parser gate design

Status: proposed for user review

Date: 2026-08-08

## Context

`scripts/test_release_workflow.py` currently uses a handwritten lexical scanner
to find GitHub Actions `uses` mappings. Successive review findings showed that
valid YAML forms such as quoted keys and flow mappings can bypass that scanner.
The same test must also prove that the release workflow retains two distinct
attestations: build provenance and an SPDX SBOM predicate.

The repository must keep the check reproducible on its Linux and macOS CI
runners. Continuing to extend the lexical scanner is not acceptable because it
would repeatedly reimplement YAML syntax without a bounded correctness model.

## Decision

Use PyYAML as a locked, test-only dependency to parse workflow files before
checking action references and structured release-step contracts.

The implementation will add a narrowly scoped requirements file containing
`PyYAML==6.0.3` with exact hashes for the Python 3.12 wheels used by the
GitHub-hosted Ubuntu x86_64 and macOS arm64 CI runners. CI will install only
binary distributions with `--require-hashes` after setting up Python 3.12 via
an already SHA-pinned `actions/setup-python` action. A missing wheel, hash
mismatch, or parse failure must fail the job.

Dependabot will gain a root pip ecosystem entry so the test-only dependency
remains visible to the weekly maintenance process.

## Parsed workflow checks

The test will load every `.yml` and `.yaml` workflow with `yaml.safe_load`.
It will recursively traverse mapping values and sequence items. Every mapping
whose parsed key is the string `uses` must contain a string that matches the
existing strict full-SHA policy: `owner/repository@` followed by exactly forty
lowercase hexadecimal characters. This preserves the current deliberate
policy: local, tag, branch, Docker, and unrecognised action references are
rejected until the policy is intentionally changed.

Because YAML comments are not part of the parsed representation, the test will
continue to assert the exact known pinned references with their adjacent
human-readable version comments. The parser gate supplies semantic completeness;
the explicit reference list preserves the review aid for the current actions.

Parsing makes bare, single-quoted, double-quoted, nested, and flow-mapping
keys equivalent. Strings inside `run` blocks, comments, and ordinary scalar
text remain strings rather than mappings and cannot become action references.

## Release-contract checks

The release workflow's job and step structure will be selected from the parsed
mapping. The contract will require a named `Attest release archive provenance`
step with the exact pinned `actions/attest` reference, the archive subject path,
and no `sbom-path`. It will separately require the existing named SBOM
attestation with the archive subject path and the generated SBOM path.

Raw-text assertions remain only where syntax matters rather than YAML structure:
the release tag shell validation and immediate revalidation before each
publication mutation. No release behavior, tag rule, archive layout, or
published release is changed by this design.

## CI and failure behavior

The Rust CI job will set up Python before invoking the release-workflow test,
then install the locked requirements file. The test command remains
`python3 scripts/test_release_workflow.py`; therefore the same failure is
visible locally after the documented dependency installation and on both CI
operating systems.

The dependency is test-only. It is not packaged into the Rust CLI, Console,
release archives, installer, or runtime execution path.

## Regression tests and acceptance criteria

The test suite must demonstrate all of the following:

- Mutable bare, single-quoted, and double-quoted `uses` keys fail in normal
  mappings, flow mappings, and nested sequences.
- `uses` text in comments, block scalars, and ordinary quoted scalars does not
  create an action reference.
- Invalid workflow YAML fails with a clear assertion.
- Removing, retagging, or giving `sbom-path` to the provenance attestation
  fails the release contract; changing the SBOM attestation inputs also fails.
- All current workflows parse, all current action references are SHA-pinned,
  and the existing release-tag revalidation checks remain intact.
- The requirements install succeeds with hashes on CI; Dependabot covers the
  new pip dependency; `cargo fmt`, Clippy, workspace tests, Console build and
  tests, release fixtures, documentation checks, YAML parsing, and
  `git diff --check` pass before merge.

## Risks and limits

Adding a parser adds supply-chain surface. Full version pinning, wheel hashes,
binary-only installation, a fixed Python version, and Dependabot mitigate that
surface. A Python or runner architecture change may require a deliberate
requirements-hash update, which is preferable to silently selecting a new
artifact.

PyYAML implements general YAML semantics, while GitHub Actions has additional
workflow-schema rules. GitHub remains the authoritative execution validator;
this test provides the repository's semantic guard for immutable action
references and the specific release contract.

## Out of scope

- Releasing a new version, changing `v0.1.0`, or running a live release.
- Changing Windows Replay/Verify support.
- Changing the already-approved GitHub repository settings plan, aside from
  adding Dependabot coverage for the new test dependency.
