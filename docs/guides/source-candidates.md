# Source candidates and installed capabilities

The unchanged public `v0.4.1` download is distinct from current main. A matching semantic version alone does not establish that an installation includes starters, batches, attempt records or Receipt comparison.

Current builds expose content-free JSON with `skilltape build-info`, `skilltape-console-api --build-info` and `GET /api/v1/build-info` (`no-store`). It records source revision, local-change flag, source/release channel, binary target and the shared `product-capabilities.json` catalog. It contains no workspace path, inputs, process arguments, environment values or saved Receipt contents. Windows still refuses replay/Verify execution; the native sandbox must also be available on supported macOS/Linux hosts. A catalog entry is not execution permission or a successful run.

In Console, expand **Installation details** to make one read of the local API and compare its identity with the built UI. Closed details make no identity request; closing/leaving aborts pending reads. A missing API, unknown source, local changes or mismatch stays unconfirmed. Reload after replacing an installation. This comparison does not authenticate a Receipt or grant execution/export authority.

## Build one local candidate

Use a clean, explicitly selected commit. Preserve the old installation and data, and place candidates in a directory named for their source revision. Build Console and both Rust binaries from that same commit:

```sh
npm --prefix apps/skilltape-console ci
npm --prefix apps/skilltape-console run build
cargo build --locked --release -p skilltape-cli -p skilltape-console-api
python3 scripts/package_release.py --version 0.4.1 --target aarch64-apple-darwin \
  --binary-dir target/release --ui-dist apps/skilltape-console/dist \
  --output-dir /selected/candidate-source-sha --require-identity
```

Select the actual native target; do not label an architecture differently. Candidate filenames retain the installer archive convention, while embedded channel/source identify them as source candidates. They are not a new public tag or replacement of `v0.4.1` assets. Debug builds can also be checked as local development candidates; do not report their startup or sizes as optimized release performance.

`--require-identity` executes only the two fixed identity commands, validates all three components against the shared catalog, and refuses dirty/unknown/mixed revisions, channels, versions or binary targets. It verifies the staged copies that will enter the archive, then includes `build-identity.json` with their digests and UI-file digests. Existing archive overwrite refusal and installer rollback remain. Legacy packaging without the flag does not assert an identity match; the current release workflow requires the flag.

A release-channel build requires the existing immutable workflow tag checks and a clean matching tag. Only the normal separately authorized release process publishes it. Source identity and hashes establish internal consistency of selected artifacts, not an independent software signature or authorship proof. No release was published by this source update.

A candidate should be installed into a new local prefix, then checked with the actual installed CLI/API/UI, first local starter, batch and Receipt inspection. Retain prior successful outputs. Installer failure must preserve/recover the prior installation; an unknown recovery state is checked before any retry. Native Windows execution, field acceptance and third-party integrations require their own evidence.
