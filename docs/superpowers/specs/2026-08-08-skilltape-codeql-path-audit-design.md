# SkillTape CodeQL Path-Safety Audit Design

**Date:** 2026-08-08

**Status:** Approved audit strategy; implementation requires review of this record
**Scope:** The 65 open CodeQL `rust/path-injection` and `py/path-injection` alerts reported by the repository's default setup.

## Goal

Resolve every current path-injection alert without weakening SkillTape's existing local-first workflow or concealing a real boundary violation. A real issue receives a minimal code fix and a regression test. An alert that is proven to be test-only or reachable only through an explicitly local, user-authorized interface receives a concise, individual audit record and, if the next CodeQL scan still reports it, an individual dismissal with an evidence-based reason. The work must not add a global CodeQL exclusion, query override, or source-code suppression.

## Context and Evidence

CodeQL default setup completed successfully on `main` with the `remote_and_local` threat model. It reported 65 high-severity path-injection alerts:

| Area | Alert numbers | Count | Initial source category |
| --- | ---: | ---: | --- |
| Tape persistence (`crates/skilltape-tape/src/store.rs`) | 55-59, 64-65 | 7 | Capture staging and local CLI paths |
| Skill template creation (`crates/skilltape-core/src/template.rs`) | 21-22, 25-26, 29-30, 63 | 7 | `#[cfg(test)]` temporary fixture path flow |
| Capture CLI (`crates/skilltape-cli/src/capture_command.rs`) | 10-16, 62 | 8 | Explicit local workspace/output and temporary paths |
| Console launcher (`crates/skilltape-cli/src/console_command.rs`) | 61 | 1 | Explicit environment override for the local API executable |
| Capture session (`crates/skilltape-capture/src/session.rs`) | 60 | 1 | Canonicalized local workspace path |
| Replay workspace (`crates/skilltape-runner/src/workspace.rs`) | 47-48, 50-54 | 7 | `#[cfg(test)]` metadata helper path flow |
| Console read model (`apps/skilltape-console-api/src/read_model.rs`) | 8-9 | 2 | Directory entries enumerated below the configured workspace root |
| Release packager (`scripts/package_release.py`) | 1-7 | 7 | Explicit release-tool command-line input paths |
| Test-only code | 17-20, 23-24, 27-28, 31-46, 49 | 25 | Test fixtures and deterministic temporary paths |

The first inspection also confirmed existing boundary controls that must be preserved:

- Replay resolves manifest-declared paths under an isolated temporary workspace, rejects non-relative paths, and rejects disallowed symbolic links.
- The Console API canonicalizes its configured root, uses fixed storage children, validates HTTP identifiers as one normal path component, and rejects symbolic links before reading documents.
- Capture and compile reject parent-directory output components, avoid silent replacement of existing output, and check output-parent symbolic links where publishing takes place.
- The release workflow supplies fixed repository-relative binary and UI directories; its `version` value is restricted to a safe component before archive naming.

These controls are meaningful evidence, but they are not treated as proof for every sink. Each alert still needs a source-to-sink review.

## Threat Model and Trust Boundaries

### Untrusted package and replay data

A skill package, its manifest fields, script references, declared outputs, and filesystem-step paths can originate outside the operator's machine. They must never cause a replay operation to access a path outside the replay workspace. The required invariant is:

```text
untrusted package path -> validate relative component -> resolve beneath workspace -> reject symlink traversal -> filesystem operation
```

This is the highest-priority boundary. A missing check, a cross-platform separator bypass, a symlink race that reaches an external path, or a path that is not demonstrably derived from `resolve_under` is a confirmed product defect.

### Console HTTP identifiers

Console route and query parameters are untrusted even when the default listener is loopback-only. They may select only a fixed storage namespace below the canonical workspace root. The required invariant is:

```text
HTTP identifier -> validate one normal component -> fixed storage child -> safe-path and symlink checks -> read-only operation
```

Any route that can turn a request value into an arbitrary filesystem path is a confirmed product defect.

### Explicit local operator paths

CLI options such as `--workspace`, `--tape`, and `--output`, plus documented Console executable/UI overrides, intentionally let the operator select local files and directories. They are not remote inputs and do not create a privilege boundary by themselves. They still require no-overwrite, confinement, canonicalization, or symlink protections wherever the command promises those semantics. The audit must distinguish an intentional local file-selection interface from an externally supplied path flowing through product data.

### Build and release inputs

`scripts/package_release.py` is invoked by a trusted release operator or the release workflow with fixed, repository-generated inputs. It is not a runtime API. The audit must nevertheless confirm that its recursive UI copy rejects symlinks and that archive names cannot be influenced through unsafe version/target components. An alert is not a vulnerability merely because a trusted release operator can choose an input directory; a real CI or repository-data path from an untrusted source would change that conclusion.

### Test fixtures

Temporary directories and filenames created only under `#[cfg(test)]` or in test files do not cross a production trust boundary. They remain subject to ordinary test hygiene, but their CodeQL alerts must be recorded as test-only rather than leading to production behavior changes.

## Chosen Approach

Three approaches were considered:

1. **Globally ignore path-injection alerts.** Rejected because it would hide a later real package, Console, or CI boundary defect.
2. **Apply generic path normalization everywhere and bulk-dismiss tests.** Rejected because it would change intentional local CLI behavior, could introduce platform regressions, and would not prove that individual sinks are safe.
3. **Evidence-driven, boundary-first remediation.** Selected. Trace every alert, fix only confirmed boundary gaps, test each corrected behavior, and record narrow reasons for genuinely controlled or test-only alerts.

The selected approach preserves product semantics while establishing an auditable answer for every existing alert.

## Resolution Rules

For every alert, the audit records the alert number, source, sink, reachable build target, trust boundary, current controls, added controls, test evidence, and final decision.

| Finding classification | Required action | CodeQL disposition |
| --- | --- | --- |
| Confirmed path escape, symlink bypass, unsafe archive member, or untrusted-to-filesystem data flow | Write a failing regression test, implement the smallest correct fix, rerun the focused and full verification gates | Leave open until the next scan verifies the detection no longer applies |
| Existing control is correct but CodeQL cannot model it | Add or retain a focused regression test and document the exact validator/safe root relationship | Individually dismiss only after review, with a factual `false positive` rationale |
| Reachable only from test code or test fixture construction | Verify the compile/test reachability and preserve the test's intent | Individually dismiss with the test-only reason; never change production code merely to silence it |
| Intentional local operator/release-tool input with no remote or privilege boundary | Document the user-controlled interface and its existing no-overwrite/symlink behavior | Individually dismiss only if no product-data or CI-controlled source reaches the sink |
| Evidence incomplete | Keep the alert open and investigate further | No dismissal |

No rule permits a bulk dismissal, a repository-wide `@codeql` suppression, a CodeQL query exclusion, or a downgrade of the CodeQL threat model.

## Implementation Shape

### 1. Establish an auditable alert ledger

Create `docs/security/codeql-path-audit.md` with one row per alert. The ledger contains no user-specific absolute paths, temporary directory values, environment values, credentials, or request payloads. It links each row to source code, tests, and the scan revision.

### 2. Review production boundaries first

Review alert groups in this order:

1. Replay workspace and package-derived paths.
2. Console read model and route-derived identifiers.
3. Capture and tape persistence, including the temporary staging directory lifecycle.
4. Template initialization and compile/output publication.
5. Console launcher overrides and release packaging inputs.

For each group, trace the value from source to sink. Existing helpers such as `resolve_under`, `ensure_no_symlink_ancestors`, `validate_id`, and canonical-root checks are evidence only when they occur on the exact path before the reported sink.

### 3. Fix only demonstrated gaps

If tracing or an adversarial test demonstrates an escape, introduce the smallest shared helper at the existing boundary rather than adding duplicate normalizers to every caller. Preserve supported absolute local CLI paths where the command explicitly documents operator choice. Prefer existing error types and fail-closed behavior.

The capture staging directory is reviewed separately for predictable-name, cleanup, and symlink-race behavior. It is replaced with a secure temporary-directory primitive only if the review or a regression test shows that the existing lifecycle fails the stated boundary; an alert alone is insufficient reason to change it.

### 4. Prove behavior before alert disposition

Add focused tests before production changes. Required adversarial cases include, where applicable:

- `..`, absolute, mixed-separator, and empty path components;
- symbolic-link roots and symbolic-link ancestors;
- a directory entry that is swapped for or resolves through a symbolic link;
- nested declared output paths and duplicate output destinations;
- Console IDs containing separators, drive prefixes, or traversal segments;
- release UI assets that include symbolic links and release version/target values with unsafe components.

Keep platform-specific symbolic-link tests correctly gated. Windows Replay/Verify remains intentionally fail-closed and is not enabled by this work.

### 5. Close alerts individually and rescan

After code and test verification, trigger or await the CodeQL scan for the branch revision. For each finding that remains and is fully evidenced as non-exploitable, dismiss that exact alert through GitHub with a short reason that references the ledger category, not an unbounded prose exception. Never include local paths, credentials, or environment contents in GitHub comments.

The final ledger records the new scan URL, commit SHA, alert state, and verification commands. A result is complete only when all 65 original alerts are either cleared by code changes or individually resolved with the recorded rationale, and no new open path-injection alert appears on the scanned revision.

## Non-Goals

- Changing SkillTape's local-first model or prohibiting documented local CLI paths.
- Enabling Windows Replay/Verify.
- Adding cloud/provider execution or distribution features.
- Altering repository visibility, branch protection, Actions permissions, CodeQL query configuration, or secret-scanning settings.
- Reformatting unrelated source or documentation.

## Risks and Mitigations

| Risk | Mitigation |
| --- | --- |
| CodeQL does not recognize a correct custom sanitizer | Preserve executable tests and record the exact source-to-validator-to-sink path; use only a narrow individual disposition after review. |
| A broad sanitizer permits a cross-platform bypass | Test path components rather than string prefixes, including Windows-style separators and drive-like input. |
| Symbolic-link checks are vulnerable to an ordering gap | Keep checks adjacent to the operation, use staging-and-rename publication, and add adversarial symlink tests where the platform supports them. |
| A local CLI behavior change breaks operator workflows | Treat absolute local paths as an intentional API unless a trust boundary proves otherwise; test the documented command behavior. |
| Audit comments disclose sensitive local details in the public repository | Use stable alert numbers and code references only; never paste command output, environment values, or absolute user paths. |
| CodeQL results lag behind a branch revision | Record the exact scanned SHA and do not claim closure until the matching run has completed. |

## Acceptance Criteria

- All 65 current alerts have one complete ledger entry and a reviewed classification.
- Every confirmed production gap has a failing regression test before its fix, followed by focused and full verification.
- Existing package, replay, Console, capture, compile, and release behavior is preserved unless a demonstrated security defect requires a bounded change.
- No global ignore, query override, threat-model downgrade, or blanket dismissal is introduced.
- GitHub alert dispositions, if needed, are individual, factual, and reference only the public audit ledger category.
- A completed CodeQL scan for the final commit has no unexplained open path-injection alert.
- The final report identifies any unresolved finding explicitly instead of presenting a partial result as complete.
