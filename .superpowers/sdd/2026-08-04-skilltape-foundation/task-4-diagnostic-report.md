# Task 4: Package Loading and Cross-File Validation

Status: complete

Implemented the complete bounded Task 4 slice:

- Added the public `Diagnostic`, `DiagnosticLevel`, and `LintReport` interfaces and crate re-exports.
- Implemented deterministic loading of the six required package files into `LoadedSkillPackage`.
- Added sanitized package errors for missing, invalid, incomplete, and unsafe package files.
- Added cross-file linting for schema, entrypoint, executable, filesystem, path-safety, input, output, and lockfile rules using stable `PKG001`–`PKG010` codes.
- Preserved the strict-mode behavior that promotes the engine-version mismatch from warning to error.

Verification:

- `cargo test -p skilltape-core --test package_validation`: 13 passed, 0 failed.
- `cargo test --workspace`: all tests passed, 0 failed.
- Rust formatting check passed for the Task 4 source files. Existing test-only formatting differences were left unchanged.

The tests used the installed rustup toolchain explicitly and a temporary writable Cargo target directory because the worktree's existing target directory is sandbox-protected and the Homebrew rustc shim lacks its old LLVM dylib.

Controller verification confirmed `package_validation` passes 13/13.

Follow-up fix (2026-08-05): Updated `PackageError::InvalidFile` to report the file and generic `parse failure` without interpolating the underlying source error. Focused test: 14 passed, 2 pre-existing diagnostic failures. Commit: `c6de124` (`fix: sanitize package parse errors`).
