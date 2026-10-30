# Saved delivery workbench contract

## Continue from package review (2026-10-30 source)

Compile and permission review now keep the selected package in their next-step
links. Export preparation loads its real workspace-relative path, suggests fresh
Receipt/output names, and provides copyable Verify and export commands. Run them
from the workspace opened by Console; Console does not execute commands or
install anything. Existing output is still protected by the export CLI.

Optionally read a saved Receipt by filename. A failed Receipt cannot be included
in prepared command text. Reading a successful Receipt does not prove it belongs
to the selected package: the export CLI remains authoritative for that binding
and final export checks. Receipt views can return to the same package/export
context. No raw package data or paths are saved in browser storage.

60 Console checks, TypeScript and production build passed. A retained synthetic
workspace was used for the real API/UI comparison: review → permissions → export
requires two links and no repeated package-path entry. The retained Receipt
stayed unchanged. Clipboard interaction was observed through an isolated mock;
the system clipboard, Skills and exports were not executed or modified.

Date: 2026-10-10. Status: implementation contract.

Distribution update — 2026-10-16: included in the public v0.2.3 Console packages.

## User outcome

A local workflow owner points Console at the parent of retained CLI delivery
directories, chooses a real saved delivery, checks its physical files and sees
what still requires independent requirement review. No demo ID or manual Receipt
copy is required. Existing Capture, compile and legacy Receipt views remain.

## Input, output and states

- `skilltape console --workspace <saved-parent>` declares the only readable root.
- `GET /api/v1/workspaces/default/deliveries?offset&limit` discovers only immediate
  non-hidden directories with delivery/Receipt metadata; it returns bounded pages
  and explicit invalid metadata. List metadata is not a physical-file check.
- `GET /api/v1/deliveries/{id}` reads the selected retained bundle and returns
  reported execution, Receipt hash binding, declared/checked file counts, bounded
  observed file metadata and findings. Requirement validation is always not-run;
  provenance remains not-authenticated.
- Empty, invalid, missing, unsafe, capacity-limited and busy states never display
  a completed physical check. A successful Receipt alone is not acceptance.

## Security and resources

### Compare a replacement before sharing

Choose **Baseline delivery** and **Replacement delivery** on the same queue
page, then **Compare file changes**. Console reuses the existing read-only
inspection API for both folders. A complete comparison requires both physical
checks and Receipt hash bindings to match, no findings, and a complete list of
at most100 files per bundle. Failed, malformed or truncated evidence reports
comparison unavailable; omitted files are never treated as unchanged.

Compare exact relative paths, observed sizes and SHA-256 hashes. Show added,
removed, changed and unchanged counts, and at most100 changed rows with an
explicit truncation notice. These are two separate local snapshots, not an
atomic observation, execution replay, authenticated baseline or business
approval. No file content is displayed. Changing either selection, refreshing
the queue or leaving the page cancels requests and discards old results.

Inspection is read-only, with no command, write, credential or provider route.
The server refuses non-loopback binding. Native no-follow directory/file handles
anchor the selected root and each member; metadata/files are rechecked before
success. Native delivery inspection currently requires POSIX; other Console
views retain their platform support.

Bound discovery to 1,000 immediate entries and pages to 100. Metadata is at most
1 MiB each. Use the existing 10,000-entry/depth64,16 MiB-file/64 MiB-total delivery
limits. Reject traversal, links, special files, duplicate/extra declarations and
credential/environment filenames before payload reads. Show at most100 file
details and64 findings with true truncation flags. No file contents, terminal
outputs, raw policy reasons or absolute roots reach the delivery DTO.

List and detail requests use the same bounded discovery catalog. HTTP input
selects actual catalog entries in memory; it does not construct filesystem
paths. Selected directories retain their discovered identity. Layout and
artifact inventories use anchored directory streams with no link following;
depth-first inspection bounds live directory handles by depth, not entry count. The
existing locked errno helper distinguishes directory exhaustion from read errors.

Receipt sections are limited to4,096 entries before schema diagnostics are
evaluated. Directory IDs are non-hidden, at most128 characters/255 UTF-8 bytes;
member paths use the existing1,024 UTF-16-unit/4,096 UTF-8-byte bound. Root ancestors
and the local operator remain trusted; these checks do not authenticate a
filesystem owner or make the service suitable for shared/untrusted hosting.

At most two blocking inspections run concurrently; excess work returns a busy
response instead of accumulating. A work slot is retained across discovery and
inspection and until actual blocking work finishes after request cancellation.
Frontend request cancellation and selection
changes release old display state; manual refresh does not create polling,
timers or unbounded history. A completed inspection is a local snapshot, not
ongoing monitoring or authentication of an author.

## Acceptance and rollback

Use a valid synthetic bundle for route/list/empty/paging/schema/size/digest and
unsafe-path tests. Reuse the retained actual CLI good/shifted deliveries for the
native API and real Chrome journey: both have intact files, while independent
SkillSync requirement checking distinguishes their allocation. Prove mutation,
missing file, invalid metadata and unsupported/busy states remain unaccepted.
Check keyboard/320px layout, selection, refresh and cancelled requests without
weakening existing scenarios. Full Rust/Console and existing packaging gates
apply. Revert normally; never alter or regenerate the successful delivery.
