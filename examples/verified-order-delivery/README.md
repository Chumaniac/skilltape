# Verified order summary delivery

A real local script aggregates three fictional orders into two daily records and
a JSON summary. SkillTape source Verify can preserve the files, Receipt and
checksummed manifest with --delivery-dir. SkillSync source artifacts checks
actual bytes, types, unique dates and cross-file count/integer-sum consistency.

This process example requires /usr/bin/perl and the existing restricted
executor on Linux/macOS; it never silently falls back to unrestricted execution.
The script limits input to 1 MiB/1,000 rows, rejects duplicate order IDs, and
requires nonnegative integer cents. These are demo rules, not general accounting
requirements. No real customer data, live service, provider or credential is used.

See the [verified delivery guide](../../docs/guides/verified-delivery.md) for
source-build commands, contract boundaries and recovery behavior. The feature
and example are local candidates, not published v0.1.0 assets.
