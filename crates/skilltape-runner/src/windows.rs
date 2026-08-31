//! Windows restricted executor — preview spike.
//!
//! Phase 0: fail-closed stub that documents the future Job Objects + ACL +
//! low integrity path. See `docs/platform-windows-sandbox.md`.
//!
//! Phase 1 will create a Job Object per replay, set `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
//! and `ActiveProcessLimit`, apply an ACL that grants only the current user on the
//! temporary workspace, and (if available) lower the token integrity to Low.
//! Until that is reviewed on a Windows runner, this module always returns
//! `SandboxUnavailable` with Windows-specific guidance. The `process.rs` layer
//! already maps `SandboxUnavailable` to a bounded `Denied` run event.

use crate::process::{ProcessError, ProcessRequest};
use std::process::Command;

/// Windows sandbox command — preview (always fails closed).
///
/// On Windows, this will eventually:
/// - canonicalize `request.cwd`,
/// - reject `allowNetwork: true`,
/// - create a Job Object with `KILL_ON_JOB_CLOSE` and `ActiveProcessLimit`,
/// - ACL the workspace to current user only,
/// - spawn with `CREATE_SUSPENDED` + `PROC_THREAD_ATTRIBUTE_JOB_LIST`,
/// - resume and wait with timeout/cancel.
///
/// For now, it returns `SandboxUnavailable` so callers fail closed with a
/// help text that points to `docs/platform-windows-sandbox.md`.
pub fn windows_sandbox_command(_request: &ProcessRequest) -> Result<Command, ProcessError> {
    Err(ProcessError::SandboxUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessRequest;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn windows_stub_always_returns_unavailable() {
        let req = ProcessRequest {
            program: "echo".to_owned(),
            args: vec!["hi".to_owned()],
            cwd: PathBuf::from("C:\\tmp\\workspace"),
            timeout: Duration::from_secs(1),
            max_output_bytes: 1024,
        };
        let err = windows_sandbox_command(&req).unwrap_err();
        assert!(matches!(err, ProcessError::SandboxUnavailable));
    }
}
