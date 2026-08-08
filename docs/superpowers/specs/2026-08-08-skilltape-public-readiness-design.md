# SkillTape public-readiness design

**Status:** Approved for execution on 2026-08-08.

## Goal

Make the already-public SkillTape repository ready for an active open-source
announcement without changing product behavior or overstating platform
support.

## Decisions

### Release integrity

Every future release must originate from an existing `v<version>` tag that
points to the workflow commit. Tag-push and manually dispatched runs use the
same validation. The workflow must use `gh release create --verify-tag`, so it
cannot create a release tag implicitly.

Every release archive must receive a GitHub build-provenance attestation and a
signed SPDX SBOM attestation. Each target-specific SBOM is distributed beside
its archive and included in `checksums.txt`. This applies to future releases;
the existing `v0.1.0` release remains a checksum-verified historical release
and must not be relabeled as attested.

All third-party and GitHub-owned workflow actions must use full commit SHAs
with an adjacent human-readable version comment. After this change is merged,
GitHub's repository setting must require SHA-pinned Actions.

### Public collaboration surface

The repository uses GitHub Issues as the initial public collaboration channel.
It adds a Contributor Covenant-based Code of Conduct, bug and feature Issue
Forms, a PR checklist, and a Dependabot schedule for Cargo, npm, and Actions.
Security disclosures remain private and are routed through GitHub Private
Vulnerability Reporting and `SECURITY.md` rather than public issues.

The README must use the real clone URL and a stable `main` documentation URL.
It must document the exact release provenance and checksum verification
boundary without claiming Windows Replay/Verify support.

### GitHub settings

`main` must require a pull request, resolved conversations, and all eight CI
and CodeQL checks. Because the repository has one maintainer, it requires zero
external approvals while administrators are subject to the same checks. Private
Vulnerability Reporting is enabled. Branch protections and the tag ruleset are
not weakened.

Repository topics improve discovery; GitHub Discussions stays disabled until a
separate support moderation process exists. A 1280×640 social-preview image
uses the local-first/replay-verifiable product message and is uploaded through
the repository settings UI.

## Non-goals

- Do not implement a Windows restricted executor or claim Windows Replay/Verify
  support.
- Do not publish crates to crates.io, create a website, or change license
  terms.
- Do not retroactively add unproven provenance to `v0.1.0`.
- Do not change CodeQL queries, severities, or threat model settings.

## Verification

Repository changes must pass release-workflow unit tests, release-package
tests, YAML parsing, Rust formatting/Clippy/workspace tests, Console build and
browser tests, documentation checks, and a fresh GitHub Actions run after
merge. Remote settings must be re-read through the GitHub API after applying
them.
