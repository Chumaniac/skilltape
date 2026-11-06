# Single verification attempt records (current source)

Use an explicit private directory to retain metadata for a standalone Verify command. Existing invocations without `--attempt-dir` keep their previous storage and stdout behavior. Published v0.4.1 downloads are unchanged.

```sh
skilltape verify examples/minimal-skill --input local-inputs \
  --attempt-dir local-run/.skilltape/attempts \
  --receipt local-run/.skilltape/receipts/check.json \
  --context local-run/.skilltape/receipts/check.json.context --json
skilltape console --workspace local-run --open
```

Choose a directory outside the selected Skill, inputs, delivery and Receipt/context targets. The CLI creates private directories on Unix and rejects existing unsafe directories or links. Only one journal writer may own the directory at a time. A full or unstable journal stops the command before verification is submitted; use a new private directory after reviewing the original records. Nothing is automatically pruned.

The journal publishes an immutable `ID.started.json` before preparation, then a separate `ID.finished.json` when an outcome can be recorded. IDs are opaque; the CLI prints the ID to stderr. Metadata contains timestamps, fixed outcome categories and digests/recorded Receipt status. It contains no original arguments, paths, environment, input text or process output. Each document is at most4KiB; a directory holds at most256 attempts and514 metadata/lease/staging entries. The completion is bound to the original start bytes. Existing metadata must remain valid before another explicit command can append.

- `preparation_failed`: this invocation was not submitted to the runner. Preparation may have created private temporary materials; this is not a guarantee of zero filesystem activity.
- `receipt_produced`: the command generated a Receipt and completed its requested Receipt/context publication. The record stores only identities and status, not another copy of the Receipt. A generated cancelled or failed Receipt remains cancelled or failed. Metadata inspection does not verify a saved Receipt file or its author.
- `unconfirmed`, or a start without completion: execution or output publication is not confirmed. It may still be active or may have been interrupted. Check the original process and retained output before explicitly starting another invocation. Opening Console never resumes or retries it.

The Receipt library has a separate **Single verification attempts** panel with explicit browsing and read cancellation. It reads at most12 summaries per page. Changing selection, cancelling or leaving the page discards late replies. Read-only `/api/v1/verify-attempts` and `/api/v1/verify-attempts/{id}` share the existing two-inspection limit, bounded no-follow readers and metadata validation. Missing completion is distinct from an unreadable/corrupt completion. Links, changed records, orphan completions, oversized documents and discovery overflow cannot establish success. Existing Receipt filenames/routes and Batch attempt/reconcile behavior remain.

Local metadata is integrity-bound, not authenticated execution evidence, frozen input proof or replay/export approval. Input fingerprints retain the existing bounded-before-run meaning. Unsupported Windows execution remains fail-closed. This source addition does not publish a release or install a runtime backend.
