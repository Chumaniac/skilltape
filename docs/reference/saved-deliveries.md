# Saved delivery workbench contract

Date: 2026-10-10. Status: source implementation contract.

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

Receipt sections are limited to4,096 entries before schema diagnostics are
evaluated. Directory IDs are non-hidden, at most128 characters/255 UTF-8 bytes;
member paths use the existing1,024 UTF-16-unit/4,096 UTF-8-byte bound. Root ancestors
and the local operator remain trusted; these checks do not authenticate a
filesystem owner or make the service suitable for shared/untrusted hosting.

At most two blocking inspections run concurrently; excess work returns a busy
response instead of accumulating. Frontend request cancellation and selection
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
