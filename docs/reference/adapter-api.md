# Exporter and Plugin API

SkillTape has two extension boundaries:

1. Built-in exporters implement the Rust `Exporter` trait and run in the same trusted process as the CLI.
2. Third-party exporters use the process protocol below and are treated as untrusted extensions. The host validates their result and runs the normal SkillTape lint gate before accepting it.

## Built-in exporter contract

```rust
pub trait Exporter {
    fn target_id(&self) -> &'static str;
    fn export(
        &self,
        package: &LoadedSkillPackage,
        output: &Path,
    ) -> Result<ExportManifest, ExportError>;
}
```

The built-in `ExportManifest` contains the target id, deterministic relative file list, and package hash. Exporters must reject an existing or unsafe output root, preserve source bytes and executable permissions, avoid network calls, and run package lint before publishing. Adapters change layout and metadata only; they do not change Workflow steps or permissions.

## Third-party process protocol

The host invokes one process for one export request:

```text
skilltape-export-plugin --input ./export-request.json --output ./exported
```

The process receives no stdin contract. It reads one UTF-8 JSON `ExportRequest` file, writes one JSON `PluginExportManifest` to stdout, writes human diagnostics to stderr, and exits with:

| Exit code | Meaning | Host result |
| ---: | --- | --- |
| `0` | manifest written | validate paths, capabilities, hash and lint |
| `2` | invalid request/input | reject without publishing |
| `3` | policy/export failure | reject without publishing |
| other/non-zero | crash or protocol failure | isolate and reject |

### ExportRequest v1

`ExportRequest.schema` must be `skilltape.dev/export-request/v1`:

```json
{
  "schema": "skilltape.dev/export-request/v1",
  "target": "example-agent",
  "input_root": "/absolute/path/to/read-only-package",
  "output_root": "/absolute/path/to/empty-output",
  "package_hash": "<64 hex characters>",
  "required_capabilities": ["metadata"]
}
```

`input_root` is an existing, non-symlink directory. The host creates or validates an empty `output_root`; a plugin must not write elsewhere. Capabilities are stable identifiers such as `metadata` or `platform-layout`, not arbitrary executable instructions.

### PluginExportManifest v1

`PluginExportManifest.schema` must be `skilltape.dev/export-manifest/v1`:

```json
{
  "schema": "skilltape.dev/export-manifest/v1",
  "target": "example-agent",
  "package_path": ".",
  "files": ["skilltape.yaml", "workflow.yaml", "permissions.json"],
  "package_hash": "<same hash as the request>",
  "capabilities": ["metadata"]
}
```

Every `files` entry is a regular, non-symlink file relative to `output_root` and must also be within `package_path`. `package_path` is `.` for a package at the output root or a safe relative directory for layouts such as `.claude/skills/<name>`. Absolute paths, drive paths, backslashes, `.`/`..` components, duplicate entries, symlinks and paths outside the requested root are rejected. The `target` and `package_hash` must exactly match the request, and every requested capability must be declared by the manifest.

After structural validation, the host loads `package_path` as a SkillPackage and runs `skilltape lint` in-process. A plugin cannot turn an invalid Workflow, permission set, lockfile, or package schema into a successful export by writing a “success” manifest.

## Authoring checklist

- Pin the protocol schema and target id; reject unknown request schemas.
- Keep stdout machine-readable JSON only; send diagnostics to stderr.
- Use deterministic file ordering and byte-preserving copies.
- Never include secrets, raw terminal output, Tape contents, or credentials in the manifest.
- Return code `2` for malformed input and `3` for policy/export rejection.
- Test missing capabilities, unknown schema, path escape, symlink output, duplicate files, invalid package lint, and plugin crash behavior before publishing an adapter.
