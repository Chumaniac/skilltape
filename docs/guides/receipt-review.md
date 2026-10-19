# Review saved verification results

Status: current source, not included in the existing v0.2.3 archives.
Date: 2026-10-19.

Save a Receipt inside the workspace you will open in Console. For an existing
Skill package, the following commands keep the saved evidence separate from
the package:

```sh
mkdir -p ./review-workspace/.skilltape/receipts
skilltape verify ./skill \
  --receipt ./review-workspace/.skilltape/receipts/review.json --json
skilltape console --workspace ./review-workspace --open
```

Choose **Verify Receipt**, enter `review` or `review.json` in **Saved Receipt
ID**, and select **Open Receipt**. The value identifies the saved filename,
not necessarily the `run_id` inside the document. Paths and traversal are
rejected; the existing local API still enforces workspace boundaries. The
picker remains available when a file is missing or unsupported, so another
saved Receipt can be opened without editing a browser URL.

**All checks** shows every recorded assertion. **Needs attention** shows failed
assertions, including a changed output digest from
[`--expect-sha256`](output-digests.md). The summary counts always cover the whole
Receipt, and filtering does not change its run status. Text labels accompany
the status colors. A run can fail or be cancelled without recording a failed
assertion; inspect its step status and policy decisions as well.

Console uses the v1 assertion `kind` field produced by the CLI. It validates
the supported document shape before rendering, rejects unknown fields, and
shows an actionable error instead of treating malformed data as a success.
Each steps/assertions/policy collection is limited to 1,000 entries; displayed
text fields are limited to 4,096 characters. Larger documents remain outside
this view. These are display limits after JSON decoding, not an API file-read
or process-memory ceiling.

The viewer only issues read requests. It does not execute a Skill, rerun
verification, alter a saved Receipt, authenticate its author, or establish
business acceptance. Generate a new Receipt through Verify when a fresh
execution is needed, preserving existing successful evidence.
