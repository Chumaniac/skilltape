# SkillTape expansion and deep optimization roadmap

Date: 2026-10-09. Status: active source implementation plan. Baseline:
`4cfc4e3790f947fbd0ec1fb40aafb9863b1e6119`. This document owns the current
expansion priorities; historical design documents retain their original dates.

## Product position and evidence

SkillTape turns a reviewed local workflow into a portable Skill and a bounded
Receipt. Its distinctive value is the connection between a captured workflow,
an isolated local replay, retained deliveries and a reviewable export. SkillSync
is an independent consumer of delivery files, not a replacement runtime.

The published baseline is GitHub v0.2.1. New work in this plan is source work
until its own release assets and provenance are independently verified. Linux
Replay/Verify require Bubblewrap and user namespaces; macOS requires
sandbox-exec. Windows Replay/Verify remain unavailable and fail closed.

## Current distribution and gaps

| Area | Existing owner | Gap to address |
| --- | --- | --- |
| Capture and Tape | capture and tape crates | Explain and test the limits of terminal/environment/file evidence. |
| Compile and package validation | compiler, core and schema crates | Keep generated declarations, paths, permissions and fixtures coherent. |
| Replay, Verify and delivery | runner, verify and CLI | Preserve restricted execution, success evidence and input limits. |
| Export | export crate and CLI | Bind recorded digests to stable copied bytes; bound traversal and copy; optionally associate a successful Receipt. |
| Console | local API and React client | Maintain paged, read-only evidence and clear unavailable/failed states. |
| Distribution | installers and release workflow | Distinguish source capabilities, current published assets and historical demonstrations. |

## This optimization cycle

### Transactional source export

Enumerate only the existing export allowlist. Limit traversal to 10,000 entries,
depth 64, 16 MiB per file and 64 MiB total. Copy observed file sizes with bounded
buffers; reject detected replacements, growth, truncation and path changes.
Calculate the existing deterministic package digest from the completed staging
tree. Validate the staged package before publishing. A completed destination
must never be replaced by another export, including after preflight.

Reuse the existing no-replace publisher across export targets. A Windows native
publisher must preserve Capture/Compile/Export usability without enabling
Replay/Verify. Windows build and publication tests are required evidence for
that branch; a portable compile alone is not Windows runtime acceptance.

### Receipt-linked export

Add an optional CLI Receipt requirement. Keep ordinary lint-only export available
on existing platforms. The guarded path checks a bounded, valid successful
Receipt against the actual staged package fingerprint, and publishes nothing
on invalid or mismatched evidence. It reports only the bounded evidence identity,
not raw Receipt/command contents. A matching externally supplied Receipt is not
authenticated execution proof. Signed/provider-backed evidence remains separate.

### Reproducible onboarding

Use current published downloads as the primary installation path; keep the
successful v0.1.0 demo explicitly historical. Add a source-only guarded export
example and an independent inventory check. Preserve old tags and assets.

## Domain expansion

### Continuation cycle — 2026-10-10

The restricted tenant-delivery example now connects an actual bounded local
producer to an independently checked physical delivery. Its expectations and
grouped outputs scope reused local IDs by organization; the SkillSync composite
contract detects balanced allocation defects. This extends the catalog without
changing source binary version0.2.2 or the existing public0.2.1 release assets.
Keep source truth, author identity, policy approval and live platforms separate.

| Domain | Concrete workflow | Useful result | Limit |
| --- | --- | --- | --- |
| Code review | Capture a local formatter/test/material collection command | Repeatable review inputs and export identity | No semantic security approval. |
| Knowledge management | Build a citation/source inventory from approved local files | Retained CSV/JSON and reviewable workflow | No source fetching or truth authentication. |
| Data operations | Run a local order/point/count reconciliation producer | Delivery and independently checked per-key values | Synthetic examples; no account or transaction execution. |
| Incident operations | Collect selected local incident materials | Bounded evidence for a later reviewer | No privileged host collection or secret discovery. |

## Priority and acceptance matrix

| Priority | Workstream | Acceptance | Dependency |
| --- | --- | --- | --- |
| P0, this cycle | Export consistency, capacity and no-overwrite publication | Regression fails before repair; final bytes reproduce the manifest digest; no partial success directory | Native filesystem primitives. |
| P1, this cycle | Optional successful Receipt association | Invalid/mismatched Receipt fails; valid staged identity passes; normal export remains compatible | Existing Receipt schema and package fingerprint. |
| P1, this cycle | CLI/docs/domain journey | Fresh local synthetic journey and CI; explicit source/release table | Current local runner where available. |
| P2 | Console evidence diff and retention policy | Bounded pages, keyboard access, empty/error states, cache release on navigation | Stable evidence APIs. |
| P2 | Compile diagnostics and domain starter library | Reproducible packages with explicit permissions and required output contracts | Domain feedback and existing standard libraries. |
| P3 | Authenticated remote delivery and Windows isolated replay | Separate threat model, runtime tests and user-approved infrastructure | Credentials, platform work and separate authorization. |

## Architecture, safety and performance

Keep the schema/core/runner/export boundaries explicit. Fix shared publication
or I/O behavior rather than patching each target independently. Preserve
deterministic output and fail-closed platform behavior. File metadata and digest
checks detect the documented concurrent changes; they do not make an unreviewed
package safe on an adversarial filesystem or authenticate its author.

Measure a fixed, within-limit export before/after with the same binary settings
and inputs. Record wall time and peak memory when the host supports them. The
streaming buffer and capacity bounds are source guarantees; they are not a CPU
benchmark. Avoid repeated whole-file allocations and retain no unbounded file
inventory, output stream or cancelled process.

## Quality, release and governance

Run new regressions, full locked Rust tests, formatting, Clippy, Console gates
and existing installer/package/workflow checks. Run the English language and
Markdown link audits. Use normal PR checks before main, without administrator
bypass. Public reports contain behavior and scope, not exploit payloads or
secret-bearing material. Update guides and CHANGELOG with exact limitations.

Future releases need a separately reviewed tag, current main binding, four
platform assets, SBOM, provenance, checksums and installed-binary journeys.
Record source, CI, published package and real-user acceptance as separate facts.
Use ordinary revert commits for rollback; do not rewrite successful assets.

## Adoption and success measures

Track first verified local delivery, successful export identity comparison,
clear platform failure, reproducible domain examples and time to review a
Receipt-linked export. Count measured outcomes rather than stars or download
claims. Paid/provider adapters and commercial packaging stay proposals until
their cost, license, trust and real workflow value are independently established.
