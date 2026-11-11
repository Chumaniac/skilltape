# Incident review material handoff

This source example prepares two fictional operations-review documents with
guarded file copies and exact SHA-256 assertions. It is material preparation
for a person, not incident analysis, production observability, or service recovery.
No process, network, model provider, or environment access is declared.

## Inputs and outputs

- Input: the bundled `incident.md` and `runbook.md` in `fixtures/input/`.
- Output: unchanged copies in `outputs/incident/`, plus a Verify Receipt.
- Review: a person checks the timeline, assumptions, and proposed next action.

The input documents are explicitly synthetic. Keep real logs, credentials,
customer identifiers, and large datasets out of the example directory. The
included fixture has two small files; the engine does not impose a general
input-directory capacity limit from this example.

## Run and adapt

Use `incident-review` as the example name in the
[domain guide](../../../docs/guides/domain-workflows.md). Missing documents or
changed contents must produce a failed run. Changing an output to an undeclared
path must be denied before the copy occurs. Expected hashes must be reviewed
alongside intentionally adapted input material.

The generic, Claude Code, Codex, and Cursor exporters produce directory layouts;
they do not demonstrate a live Agent or an OS process sandbox. This example is
a local source candidate and is not included in public v0.1.0 release assets.
Use new Receipt and export destinations; existing paths are not overwritten.
