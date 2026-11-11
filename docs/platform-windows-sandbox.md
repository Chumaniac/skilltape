# Windows Restricted Executor — Design Spike (Fail-Closed Preview)

> Status: Spike / Preview — Not yet claiming isolated replay on Windows.
> Replay/Verify intentionally fail closed until the design below is reviewed and tested on Windows runners.

## Problem

Linux uses `bubblewrap` + user namespace, macOS uses `sandbox-exec`. Windows has no equivalent in the current `process.rs` and falls through to `SandboxUnavailable` (fail-closed). Releases still ship Windows archives/Capture/Compile/Lint/Export, but consumers expect Replay parity.

## Goals

- Keep fail-closed default: never simulate isolation on Windows.
- Provide a clear, actionable error (`sandbox_unavailable` with Windows guidance) instead of a generic “not supported”.
- Define a Windows isolation path that can be implemented incrementally without breaking Linux/macOS.
- Make the security posture auditable: document what each stage blocks and what it does not.

## Non-Goals (Preview)

- Claiming Windows isolation is equivalent to Linux/macOS before review.
- Replacing general process supervision (`process.rs` timeouts, output truncation, cancellation) — those stay platform-agnostic.
- Automatically granting network/filesystem beyond the temporary workspace.

## Threat Model (Windows)

Same as `SECURITY.md`: untrusted Tape content tries to
- read/write outside the temporary workspace,
- spawn unbounded child processes,
- exfiltrate via network,
- retain secrets in logs.

Windows-specific risks: ACL inheritance, Integrity Levels, Job Objects cleanup, NTFS symlink reparse points, PowerShell `ExecutionPolicy`.

## Design: Job Objects + ACL + Integrity Level

### Phase 0 — Keep fail-closed, improve diagnostics (this spike)

- `process.rs`: on `#[cfg(target_os = "windows")]` keep `SandboxUnavailable` but surface a Windows-specific help text:
  ```
  Windows Replay/Verify requires a restricted executor (Job Objects + low integrity + ACL). See docs/platform-windows-sandbox.md.
  Supported now: Capture/Compile/Lint/Export. Replay/Verify will fail closed with code sandbox_unavailable.
  ```
- `crates/skilltape-runner/src/windows.rs` (new): documents the future executor interface, does not yet claim isolation. Contains `WindowsExecutor` stub that always returns `SandboxUnavailable`.
- `SECURITY.md`: update Windows row to “Supports non-execution commands; Replay/Verify fail closed with preview design linked”.
- CI: keep `windows-latest` in the packaging matrix but not in the Replay matrix; add a unit test that asserts Windows returns `SandboxUnavailable`.

### Phase 1 — Minimal isolation preview (requires Windows runner)

- **Job Object** per replay:
  - `CreateJobObjectW` + `SetInformationJobObject` with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`,
  - `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` (ceil = `ResourceLimits.max_processes`),
  - `JOB_OBJECT_LIMIT_JOB_MEMORY` if needed,
  - `JOBOBJECT_BASIC_LIMIT_INFORMATION::ActiveProcessLimit`.
  - Assign each child via `AssignProcessToJobObject`.
- **Filesystem**: workspace is a fresh `TempDir` under `%RUNNER_TEMP%` or `GetTempPathW`; ACL via `SetNamedSecurityInfoW` removing `BUILTIN\Users` write on parent, granting only current user. Deny `..` traversal by canonicalizing `cwd` first.
- **Integrity**: child token via `SaferComputeTokenFromLevel` or `CreateRestrictedToken` + `SetTokenInformation(TokenIntegrityLevel)` to Low if available; fallback to Medium + explicit deny if Low not available (still fail-closed if elevation cannot be lowered).
- **Network**: `Windows Filtering Platform` is out of scope for Phase 1; instead, rely on policy layer: `skilltape_policy::PolicyEngine` already denies `allowNetwork:false`. Preview will reject any Tape with `allowNetwork:true` on Windows rather than silently allowing.
- **Error handling**: every Win32 call checked; on any `GetLastError != 0`, tear down job and return `SandboxUnavailable` with OS error prefix (redacted, no secret leakage).

### Phase 2 — Hardening (future)

- Add `LPSECURITY_ATTRIBUTES` + `PROC_THREAD_ATTRIBUTE_JOB_LIST` for child creation,
- Add `JOB_OBJECT_SECURITY_LIMIT_INFORMATION` to block `ADMIN`/`SYSTEM` handles,
- Add `AppContainer` or `LPAC` if the runner supports it (requires manifest change).

### Execution Flow (Preview)

```
verify --receipt
  -> run_skill()
     -> TokioProcessAdapter::run()
        -> sandboxed_command()  // on Windows: windows::windows_sandbox_command()
           -> Validate workspace canonicalization
           -> Validate policy.allowNetwork == false else SandboxUnavailable
           -> Create Job Object, try to set limits
           -> If any step fails => SandboxUnavailable + docs link
           -> Else (future) spawn with CREATE_SUSPENDED, assign to job, resume, wait with timeout/cancel
```

### Exit Codes and Evidence

- `ProcessError::SandboxUnavailable` maps to `RunEvent.status = Denied` with `exit_code: None` and stderr containing the Windows help text (bounded, no secrets).
- Receipt records `status: failed` + `policy: denied` for Windows replay attempts; no change to Linux/macOS.

## Interfaces

```rust
// crates/skilltape-runner/src/windows.rs
pub fn windows_sandbox_command(request: &ProcessRequest) -> Result<Command, ProcessError> {
    // Phase 0: always SandboxUnavailable with help text
    // Phase 1: try Job Object; on failure, still SandboxUnavailable
}
```

```rust
// crates/skilltape-runner/src/process.rs
#[cfg(target_os = "windows")]
fn sandboxed_command(request: &ProcessRequest) -> Result<Command, ProcessError> {
    crate::windows::windows_sandbox_command(request)
}
```

## Testing

- Unit: `cargo test -p skilltape-runner -- windows` — assert `windows_sandbox_command` returns `SandboxUnavailable` on non-Windows (or on Windows without Job Object).
- Integration: `cargo test -p skilltape-cli --test integration_full_journey` remains Linux/macOS-only; Windows run is `#[ignore]` with `reason = "Windows sandbox preview not yet ready"`.
- Manual: on a Windows 11 runner with `cargo run -p skilltape-cli -- verify <pkg> --json` expect `sandbox_unavailable` + `docs/platform-windows-sandbox.md`.

## Rollback

- Any failure in Windows path returns `SandboxUnavailable`; no partial isolation.
- Feature is behind `cfg(target_os = "windows")`; Linux/macOS unchanged.
- Docs remain explicit: Preview does not satisfy `SECURITY.md` threat model for Windows isolation.

## Alternatives Considered

- **WSL**: requires optional Windows feature, not guaranteed on CI, adds Linux dependency.
- **AppContainer alone**: needs package identity, not suitable for CLI-spawned processes without manifest.
- **Doing nothing**: keeps generic error, hides design and blocks contribution.

## References

- `crates/skilltape-runner/src/process.rs` — current `SandboxUnavailable`
- `SECURITY.md` — platform table
- `crates/skilltape-runner/src/windows.rs` — stub + future design
