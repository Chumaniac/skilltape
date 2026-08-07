# Documentation style

This guide defines the English documentation standard for SkillTape. English
is the canonical source language for all natural-language content in tracked
files, including Markdown, examples, fixtures, workflow descriptions, and
user-facing copy.

## English-only prose

- Write explanatory prose, headings, comments, descriptions, status labels,
  examples, error copy, and documentation metadata in English.
- Keep product names and technical values exact: SkillTape, Tape, Skill,
  Console, Capture, Compile, Lint, Replay, Verify, and Receipt are product
  vocabulary, not terms to translate or replace with synonyms.
- Keep arbitrary fixture payloads unchanged when they test bytes or protocol
  values rather than document product behavior. If a language scan reports an
  unrelated Unicode symbol, record why it is not natural-language content.

## Headings and structure

- Use sentence-case headings: capitalize the first word and proper names, not
  every significant word.
- Give each document one descriptive top-level heading and use heading levels
  in order.
- Prefer short paragraphs, descriptive lists, and tables when they make a
  contract or comparison easier to scan.
- Mark historical design, plan, and report material as archival when its
  status is no longer current; preserve the recorded evidence.

## Product vocabulary

Use the established terms consistently:

| Term | Meaning and usage |
| --- | --- |
| SkillTape | The product and repository name. |
| Tape | A captured, redactable record of terminal and filesystem work. |
| Skill | A compiled, linted package with a declared Workflow and permissions. |
| Capture | The operation that records a workflow into a Tape. |
| Compile | The deterministic operation that turns a Tape into a Skill package. |
| Lint | Structural, path, permission, policy, and lockfile validation. |
| Replay | Running a Skill in an isolated workspace. |
| Verify | Running Replay, assertions, and Receipt generation. |
| Receipt | Bounded evidence describing a Verify result. |
| Console | The optional read-only local viewer for Tapes, Skills, runs, and Receipts. |

Also use `permission policy`, `redaction`, `fixture`, `sandbox`, `Workflow`,
and `evidence` consistently. Keep command names in their exact form, such as
`skilltape capture`, `skilltape compile`, and `skilltape verify`.

## Normative and informative wording

- Use **must** and **must not** for requirements and invariants.
- Use **should** and **should not** for recommendations that permit a
  documented exception.
- Use **may** for an allowed option or a possible outcome; do not use it for a
  requirement.
- Separate normative rules from informative explanations and examples. Label
  examples as examples, and do not present historical evidence as a current
  guarantee.
- State security, compatibility, and release limitations explicitly instead
  of implying broader support than the implementation provides.

## Commands and code blocks

- Preserve commands, CLI flags, paths, URLs, schema identifiers, protocol
  field names, package names, target triples, environment variables, hashes,
  timestamps, and fixture values exactly.
- Preserve fenced code blocks, their language tags, indentation, quoting, and
  line order. Translate only surrounding prose or a natural-language comment
  when the comment itself is part of the requested documentation change.
- Keep examples runnable. If a command or output changes because product
  behavior changed, update the corresponding guide, test, and release evidence
  together; a language migration alone must not change behavior.

## Relative links

- Link to repository files with relative Markdown targets, resolved from the
  file containing the link. Use descriptive link text and preserve anchors
  when they identify the intended section.
- Use full `https://` URLs only for intentionally external resources. Do not
  use `file://` URLs, machine-specific absolute paths, or links to an
  untracked worktree file.
- When moving or renaming a document, update every inbound relative link in
  the same change and check links from historical documents as well as public
  entry points.

## Status and dates

- Give specifications, plans, release-readiness records, and historical
  reports an explicit status and an ISO date (`YYYY-MM-DD`) when their format
  calls for metadata.
- Preserve historical dates, task identifiers, commit identifiers, evidence,
  acceptance criteria, and limitations. Add an archival note when current
  status needs clarification; do not silently rewrite historical evidence.
- Describe release state precisely: distinguish merged implementation evidence
  from a published versioned GitHub Release, and distinguish required future
  steps from completed checks.
- Refresh status claims against the current implementation and CI evidence
  before changing a public document.

## Security-copy review

Before merging security-sensitive documentation or user-facing copy, compare
it with the implementation and `SECURITY.md`:

- Check sandbox boundaries, permission defaults, path restrictions, redaction,
  secret handling, and the supported platform matrix.
- Do not claim that raw Tapes, Receipts, logs, environment values, or provider
  credentials leave the machine unless the implementation explicitly does so.
- Keep warnings actionable and avoid turning a bounded local guarantee into a
  general security promise. Recheck Console, installer, release, and workflow
  messages when their copy changes.

## Required language and link checks

Run the repository-wide language scan for every documentation change and
review every match. The scan intentionally excludes `Cargo.lock` because it
is generated dependency data:

```bash
git grep -n --perl-regexp '[\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}]' -- . ':!Cargo.lock' || true
```

The final gate must fail if the scan reports a natural-language match:

```bash
if git grep -n --perl-regexp '[\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}]' -- . ':!Cargo.lock'; then
  exit 1
fi
```

Run this repository-local Markdown link audit from the repository root. It
checks relative targets, ignores anchors and external URLs, resolves targets
relative to their source file, and fails for a missing target:

```bash
python3 - <<'PY'
from pathlib import Path
import re
import subprocess
import sys

link_pattern = re.compile(r'(?<!!)\[[^\]]+\]\(([^)\s]+)(?:\s+["\'][^"\']*["\'])?\)')
files = subprocess.check_output(["git", "ls-files", "-z", "--", "*.md"]).split(b"\0")
missing = []

for raw_path in files:
    if not raw_path:
        continue
    source = Path(raw_path.decode())
    text = source.read_text(encoding="utf-8")
    for match in link_pattern.finditer(text):
        target = match.group(1).strip("<>")
        if not target or target.startswith("#"):
            continue
        if re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", target):
            continue
        target_path = target.split("#", 1)[0].split("?", 1)[0]
        if target_path and not (source.parent / target_path).exists():
            missing.append(f"{source}: {target}")

if missing:
    print("Missing Markdown targets:", file=sys.stderr)
    print("\n".join(missing), file=sys.stderr)
    raise SystemExit(1)
PY
```

Also run `git diff --check` before committing. A documentation change is
ready only when the language scan has no natural-language matches, all local
links resolve, and the diff has no whitespace errors.
