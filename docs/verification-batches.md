# Verify multiple input sets with resumable batches

Current source builds add a private, local batch workspace. Published v0.4.1
downloads are unchanged. Review the Skill and its permissions before preparing
the batch. Preparation copies bounded Skill/input snapshots and executes nothing.

```json
{
  "schema": "skilltape.dev/batch-plan/v1",
  "skill": "skill",
  "cases": [
    {"id": "north", "input": "inputs/north", "expected_files": ["outputs/report.json"]},
    {"id": "south", "input": "inputs/south", "expected_files": ["outputs/report.json"]}
  ]
}
```

Paths are relative to this plan's folder and cannot contain `..` sequences,
links or absolute paths. A case may also declare `expected_hashes` with the same
`PATH=SHA256` syntax as `verify --expect-sha256`. The output workspace must be new
and outside the source Skill and input folders.

```sh
skilltape batch prepare materials/batch.json --output batch-workspace
skilltape batch run batch-workspace --yes --json
skilltape batch inspect batch-workspace --json
skilltape console --workspace batch-workspace --open
```

Cases run serially through the existing verifier, sandbox and output assertions.
Each attempt gets a separate Receipt, run context and, when successful, a
no-overwrite delivery. Console's batch page links current and earlier attempts;
it issues read requests only. Inspect the linked delivery to check actual files.

Running the batch again executes only pending cases. A failed or cancelled case
needs an explicit retry. A corrected input folder creates a new frozen snapshot;
the prior input, failure evidence and all successful attempts remain unchanged.

```sh
skilltape batch retry batch-workspace --case south --input corrected-south --yes
```

Without `--input`, a retry uses the existing frozen input. Changed frozen
materials or saved evidence block continuation. Unknown/interrupted outcomes do
not automatically retry: `batch reconcile WORKSPACE` reads complete, bound saved
evidence and recovers its recorded state when possible. Missing or inconsistent
evidence stays unconfirmed and requires inspection; it cannot become success.
Do not edit the journal to make it pass or rerun unknown work to replace it.

The source protocol permits 16 cases, 3 attempts per case and 256 KiB per plan or
journal. Frozen Skill/inputs share 128 MiB and 10,000-file ceilings. Admission
reserves 66 MiB for the next delivery within a 256 MiB retained-output budget.
This is an admission check, not an operating-system disk quota: an exceptional
assembly failure may retain additional private material for inspection and stops
the batch. Individual execution/delivery limits remain in force. No old result
is deleted to recover capacity. Larger programs require explicit retention design.

Exit 0 means a completed batch, successful preparation or read operation; exit 3
means known incomplete/failed verification, and exit 5 means cancellation. Invalid
or unsafe materials, capacity failures and blocked unknown outcomes return 2 with
a diagnostic. Input snapshots may contain private business files: keep the private
workspace protected. Summaries omit raw file contents, command output and original
absolute paths. Integrity hashes do not authenticate an author or prove business
acceptance; review the actual declared checks and use a SkillSync contract when
business acceptance is required.

The workflow borrows declaration, isolated queue and explicit continuation ideas
from [DVC experiment queues](https://doc.dvc.org/command-reference/exp/run), examined
at [treeverse/dvc@56e5982](https://github.com/treeverse/dvc/tree/56e59829512ff134aa269099a2099587b810b4dd)
(Apache-2.0). Implementation reuses SkillTape's own verifier and delivery format;
no DVC code, runtime, dependency or remote service is included.
