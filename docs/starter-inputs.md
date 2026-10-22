# Starter inputs

These materials have not been executed. Edit `inputs/request.json`, then run
`skilltape starter run PATH --output NEW_RUN_DIRECTORY --yes`. The output's parent
must exist. Use a new directory outside these materials for every run.

## File inventory

Put files in `inputs/files/`. List each plain file name and its reviewed SHA-256
in `files`: `{"files":[{"path":"sample.txt","sha256":"64 hexadecimal characters"}]}`.
A missing file or changed hash stops the task. Review a changed source before
updating the expected hash. Nested names, links, duplicates and empty lists are
rejected. Maximum: 128 files, 1 MiB per file; the copied input tree also has a
64 MiB total limit. Files not listed are not certified by the report.

## Quantity reconciliation

Use `expected` and `received` arrays of `{"item":"part-1","quantity":5}`.
Duplicate item rows are summed. Missing items have quantity zero on that side.
The report lists expected, received and their difference. `balanced: false` is a
valid reconciliation result, not a Verify failure. It means the task reproduced
the discrepancy; it does not mean the quantities agree. Use at most 1,000 rows
per side, nonnegative integer quantities up to 1,000,000,000, and item names of
1 to 128 UTF-8 bytes. The request must fit in 1 MiB. CSV is not accepted.

## Results and failures

A run snapshots inputs, executes one fixed local producer through Capture,
compiles the actual Tape, lints the package, re-executes it through Verify in the
platform sandbox, checks the result hash, and exports it with the real Receipt.
The producer is the installed SkillTape binary; keep that version available.
There are no remote templates, shell commands, hooks, provider calls or network.
macOS and Linux require the supported sandbox. Windows execution stays disabled.

Read `deliveries/task/artifacts/report.json` for the result, and
`deliveries/task/receipt.json` for verification evidence. Open the run with
`skilltape console --workspace RUN_DIRECTORY --open`; Console is read-only.
The generic export contains the Skill and Receipt, not private input files.

A failed stage retains preceding results and never reports a completed delivery.
Correct the named input or stage, then choose a new run directory. Existing run
folders and successful outputs are never overwritten. Ctrl-C cancels Capture or
Verify and uses their existing process-group cleanup.
