# Review capture coverage

Open **Capture timeline**, select a saved Tape and click **Check whole Tape**. The check counts stored redaction labels, event kinds, sources and capture warnings across the complete validated stream. It runs only when requested; changing event pages does not scan again.

The overview returns metadata, not event payload or the recorded workspace path. Status labels cannot establish whether content is safe to share. Inspect the original events and retain their evidence before sharing a recording.

## Follow a sequence reference

The overview retains at most 32 sequence references for events labelled unredacted or partially redacted, or carrying a capture warning. A sequence link opens the existing 100-event page containing that zero-based sequence. Full counts remain visible when the reference list is truncated.

## Read snapshot and failure states

A stable finished capture is labelled a finished snapshot. Open captures may continue recording; a manifest that changes during the check is labelled changed. Recheck when a stable snapshot is needed.

The same Tape path, manifest, JSONL, event-count and sequence checks apply to the event view and coverage view. The scan uses the existing 16 MiB and 100,000-event budgets. Invalid or oversized recordings return an error rather than partial success. Two concurrent inspection slots bound active work even when an HTTP caller disconnects.

The read-only resource is `GET /api/v1/tapes/{id}/coverage`, with schema `skilltape.dev/tape-coverage/v1`. Counters and bounded sequence references describe the observed capture only; they do not authorize a workflow replay or confirm business requirements.

Both event-page and coverage reads resolve the checked Tape path and confirm it stays under the chosen workspace's Tape directory before shared manifest or stream access. Existing no-symlink and single-component identifier checks still apply; safe dotted identifiers remain supported.

A manifest must be a regular file. Missing manifests keep their not-found response; invalid types and unreadable metadata produce a bounded invalid-document response without forwarding filesystem error objects into the selected store.
