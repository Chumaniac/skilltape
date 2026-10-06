# Verified local delivery

Status: Local implementation candidate.
Date: 2026-10-06.

## User outcome

A completed workflow can produce a persistent, independently inspectable delivery
instead of losing its output files when the CLI temporary workspace is removed.
SkillSync checks actual delivery bytes against explicit CSV/JSON requirements and
cross-file totals. A synthetic order summary demonstrates the complete flow.

## Producer contract

`skilltape verify --delivery-dir <new-directory>` preserves a successful Verify in
one atomically published directory. It conflicts with standalone `--receipt`.
Default Verify and all existing v1 Receipt/Replay documents remain unchanged.

The bundle contains `receipt.json`, `delivery.json`, and `artifacts/`. The manifest
schema is `skilltape.dev/delivery/v1` with run_id, skill_hash, receipt_sha256,
artifact_set_sha256, and sorted file records `{path, bytes, sha256}`. Paths are
relative to artifacts; no stdout, stderr, environment, or absolute host path is
added. Manifest records are sorted by UTF-8 path bytes. The set hash concatenates
path UTF-8 bytes, NUL, unsigned 64-bit big-endian size, and the 64 ASCII digest
characters for each file. The Receipt hash covers its exact persisted bytes.

Linux/macOS publication uses native no-replace rename, including when an empty
destination appears after preflight. Existing targets, symlink ancestors, parent
components, and destinations overlapping the package/input are rejected. A
failed workflow never publishes a bundle. If assembly/publication fails after a
successful Verify, completed artifacts remain in a private sibling staging
directory; the CLI identifies that generated basename for recovery.

Delivery assembly limits are 10,000 entries, depth 64, 16 MiB per artifact,
64 MiB total artifact bytes, and 1 MiB per metadata document. Hashes stream through
8 KiB. These bounds cover delivery assembly, not an OS filesystem/process quota.

## Consumer contract

`skillsync artifacts --delivery <directory> --contract <json-or-yaml> --format json`
performs read-only physical inspection. It runs no Skill, Replay, Docker, provider,
or remote service. Exit 0 means the declared checks passed; 1 means an artifact
check failed; 2 means input/contract error. Existing test/Replay semantics remain.

`skillsync.artifacts/v1` contracts declare explicit lower capacity limits, required
file paths, optional expected digests, bytes/CSV/JSON formats, and optional
CSV-to-JSON row-count/integer-sum comparisons. CSV rules include exact header
order, scalar column types, row bounds, uniqueness, and formula-prefix rejection
by default. JSON rules check declared top-level field types, required values, and
optional exclusion of extra fields. No expressions, arbitrary scripts, network
references, or full JSON Schema interpretation are enabled.

Only declared regular files are read. Structural checks reject extra files,
symlinks, special files and capacity overruns before reading payloads. File reads
are bounded, and changes during the snapshot are rejected. Reports contain
relative paths, hashes, sizes, record counts and stable issue codes, never raw
cells, JSON values or local absolute paths. Valid metadata consistency is not a
signature, authenticated provenance, proof of input truth, or general business
correctness.

## Sequential slices and acceptance

1. Producer: persistent files/manifest/Receipt; no overwrite, overlaps or links;
   failed-run nonpublication; assembly/publication recovery; unchanged defaults.
2. Consumer: actual hashes; UTF-8 CSV quoting/multiline/escaped quotes; schema,
   duplicate/formula/unsafe-integer cases; JSON and cross-file mismatch; bounded
   reads, extra files, links, changed files and report privacy.
3. Synthetic order summary: real local script generates CSV/JSON, producer stores
   the bundle, consumer passes it then rejects corruption and inconsistent totals.
   Update detailed project overviews with the exact source/published boundaries.

All work stays local until separately authorized publication. Existing incident
review candidates, original user workspaces, and successful synthetic assets are
preserved. Real providers, production data, new services and package releases
are outside this implementation.
