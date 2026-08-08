# User-first Documentation and Five-minute Quickstart Design

**Status:** Approved design

## Goal

Make the public documentation answer one question before anything else:

> What can I do with SkillTape immediately after I install it?

Within the first three README sections, a new visitor must be able to identify
the product value, see a real result, understand the Beta boundary, and copy a
native-release install and first-run path that produces a useful local result
within five minutes.

The primary promise is:

> Turn a real local workflow into a reviewable Agent Skill you can replay and
> verify before you share it.

## Decisions

### Native release distribution, not npm

SkillTape remains a native Rust CLI. The public entry point uses the existing
GitHub Release installer and a fixed release version; it does not introduce an
npm wrapper or claim that `npx` can run the product.

The Unix quickstart uses the existing checksum-verifying installer downloaded
from a fixed release tag. Windows uses the existing PowerShell installer. The
documentation must name the operating-system path plainly and must never imply
that the Unix script supports Windows.

### An honest visual demo, not a fake hosted service

The README embeds one accessible terminal-style screenshot generated from a
real, deterministic Capture → Compile → Lint → Verify run. It links to the
matching text transcript and the minimal Skill fixture. The call to action is
"Watch the demo", not a claim that a hosted SkillTape service exists.

The visual is a versioned SVG or PNG asset paired with its source transcript.
It must show only redacted, reproducible output from the repository fixture;
no private paths, credentials, hand-authored success output, or fictional UI
are permitted.

### Beta is visible before the first command

The README labels the project **Beta** directly below the value statement and
states the product boundary in ordinary language:

- Linux and macOS support the full restricted Capture → Compile → Lint →
  Replay → Verify path when their documented sandbox prerequisite is present.
- Windows supports Capture, Compile, Lint, and Export. Replay and Verify stop
  safely because there is not yet an equivalent restricted executor.
- The product is local-first; no cloud service, model provider, API key, or
  configuration file is required for the first result.

The boundary appears before advanced installation and never hides a failed
platform capability behind an optimistic command.

## User experience

### README first screen

The public README order is:

1. Product name, one-sentence value, and a Beta label.
2. A short outcome-focused paragraph that says who should use SkillTape and
   what it produces: a reviewable Skill and a Receipt from a local workflow.
3. The real terminal screenshot, a "Watch the demo" link, and a direct
   release-download link.
4. A five-minute quickstart block with no prerequisite architecture reading.
5. A compact "Use it when / not yet for" boundary, including the Windows
   Replay/Verify limitation and the local-artifact privacy reminder.
6. Links to Quickstart, installation, configuration, reference, security, and
   contributing material only after the first-result path.

The README must not lead with Rust, Node.js, CI, release mechanics, internal
architecture, future plans, audit reports, or file-by-file explanations.

### Five-minute result

The Unix path has two copyable blocks:

1. Download the installer from the fixed `v0.1.0` source tag, use the public
   GitHub Release base URL and fixed version, and install into the documented
   local bin directory. The installer verifies the selected release archive
   against `checksums.txt` before replacement.
2. Create a temporary workspace and run the smallest deterministic workflow:
   Capture a harmless command, Compile it into a Skill, Lint it, then Verify it
   and show the resulting Receipt path or JSON summary.

The Windows path mirrors the first result through Capture → Compile → Lint →
Export and explicitly says why Verify is omitted. It must not ask a user to
install a compiler or build the repository.

If the local sandbox prerequisite is missing, Quickstart must explain the
smallest platform action needed to enable Verify and still show the commands
that remain usable without it.

### Configuration is secondary and factual

The first run needs no configuration. A separate short configuration section
uses shell environment-variable templates only for settings the product
actually reads:

- fixed release version, target, install directory, and release base URL for
  installation;
- `SKILLTAPE_CONSOLE_API_BIN` and `SKILLTAPE_CONSOLE_UI_DIST` only when a user
  intentionally overrides packaged Console discovery.

The documentation must state that SkillTape does not auto-load a `.env` file;
the examples show `export` or PowerShell environment assignments rather than
inventing a configuration format.

## Documentation structure

Public navigation is task-oriented:

```text
README.md
  ├─ Quickstart: create and verify a first Skill
  ├─ Install and update SkillTape
  ├─ Optional configuration and local Console
  ├─ Examples and format reference
  ├─ Security policy
  └─ Contributing
```

`docs/README.md` and `docs/guides/README.md` use the same user verbs rather
than catalogs of individual files. `docs/guides/quickstart.md` owns the
expanded explanation of the first-result commands. `docs/guides/installation.md`
owns platform install, update, and troubleshooting details; it no longer uses
"local CI" as part of its title or opening premise.

Historical plans, release-readiness material, audits, specifications, and
execution reports remain available for maintainers and contributors but are
not linked from the primary user path or described as prerequisites for use.
They are preserved rather than deleted.

## Content that moves out of the user path

Remove or demote from README and user indexes:

- pre-publication readiness narratives;
- exact CI and GitHub Actions implementation descriptions;
- benchmark and architecture detail unrelated to a first run;
- complete product plans, design specifications, and execution reports;
- inventory-like descriptions of what each repository file contains.

Keep security boundaries, release-install integrity guidance, and platform
limitations where they help a user make a safe decision.

## Verification

The implementation must provide evidence for the user-facing claims:

1. Run the exact documented Unix installation flow in a fresh temporary
   directory and confirm it installs the `v0.1.0` release after checksum
   verification.
2. Run the exact Linux/macOS Quickstart commands in a temporary workspace and
   confirm that a Tape, Skill, and Receipt or JSON verification summary are
   produced.
3. Run the documented Windows-safe path in its platform fixture or command
   contract and confirm it never claims Replay/Verify success.
4. Check every Markdown link and asset reference, including alt text and the
   visual-demo transcript link.
5. Add a lightweight documentation contract test for the value statement,
   Beta label, visual asset, quickstart link, direct release link, and the
   absence of retired user-entry headings.

## Non-goals

- Publishing an npm launcher or hosted demo service.
- Changing Capture, Compile, Replay, Verify, or Console behavior.
- Enabling Windows Replay/Verify.
- Deleting historical plans, reports, or design records.
- Presenting build-from-source instructions before a user can get a first
  result.
