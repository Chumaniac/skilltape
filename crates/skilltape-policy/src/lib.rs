mod risk;
mod rules;

use serde::{Deserialize, Serialize};
use skilltape_schema::Permissions;

pub use risk::RiskLevel;
pub use rules::{FileAccess, PolicyRules};

/// Stable codes used by compiler, lint, and runtime policy decisions.
pub mod codes {
    pub const ALLOWED: &str = "POLICY_ALLOWED";
    pub const MISSING_PACKAGE_FILE: &str = "PKG001";
    pub const INVALID_ENTRYPOINT: &str = "PKG002";
    pub const INVALID_PACKAGE_SCHEMA: &str = "PKG003";
    pub const INVALID_COMMAND: &str = "POLICY_COMMAND_INVALID";
    pub const DANGEROUS_COMMAND: &str = "POLICY_COMMAND_DANGEROUS";
    pub const UNDECLARED_EXECUTABLE: &str = "PKG004";
    pub const READ_SCOPE: &str = "PKG005";
    pub const WRITE_SCOPE: &str = "PKG006";
    pub const UNSAFE_PATH: &str = "PKG007";
    pub const UNDECLARED_INPUT: &str = "PKG008";
    pub const UNDECLARED_OUTPUT: &str = "PKG009";
    pub const LOCKFILE_MISMATCH: &str = "PKG010";
    pub const INVALID_HOST: &str = "POLICY_NETWORK_INVALID_HOST";
    pub const NETWORK_DISABLED: &str = "POLICY_NETWORK_DISABLED";
    pub const HOST_NOT_ALLOWED: &str = "POLICY_NETWORK_HOST_NOT_ALLOWED";
    pub const INVALID_ENVIRONMENT: &str = "POLICY_ENVIRONMENT_INVALID";
    pub const ENVIRONMENT_DISABLED: &str = "POLICY_ENVIRONMENT_DISABLED";
    pub const SECRET_IDENTIFIER: &str = "POLICY_SECRET_IDENTIFIER";
}

/// The result of one deterministic capability check.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub code: String,
    pub reason: String,
    pub risk: RiskLevel,
}

/// Pure policy evaluation over a package's declared permissions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyEngine {
    rules: PolicyRules,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new(PolicyRules::default())
    }
}

impl PolicyEngine {
    pub fn new(rules: PolicyRules) -> Self {
        Self { rules }
    }

    pub fn rules(&self) -> &PolicyRules {
        &self.rules
    }

    pub fn check_command(
        &self,
        program: &str,
        args: &[String],
        permissions: &Permissions,
    ) -> PolicyDecision {
        if program.is_empty()
            || program.trim() != program
            || program.chars().any(|character| character.is_control())
        {
            return decision(
                false,
                codes::INVALID_COMMAND,
                "command identifier is invalid",
                RiskLevel::High,
            );
        }

        let normalized_program = command_name(program);
        let command_line = command_line(&normalized_program, args);
        if self.rules.denies_program(&normalized_program)
            || self.rules.denies_argument_fragment(&command_line)
            || is_dangerous_command(&normalized_program, args)
        {
            return decision(
                false,
                codes::DANGEROUS_COMMAND,
                "command matches a dangerous policy rule",
                RiskLevel::Critical,
            );
        }

        if !permissions
            .process
            .executables
            .iter()
            .any(|declared| declared == program)
        {
            return decision(
                false,
                codes::UNDECLARED_EXECUTABLE,
                "executable is not declared by package permissions",
                RiskLevel::High,
            );
        }

        decision(true, codes::ALLOWED, "allowed", RiskLevel::Low)
    }

    pub fn check_path(
        &self,
        path: &str,
        access: FileAccess,
        permissions: &Permissions,
    ) -> PolicyDecision {
        if !is_safe_workspace_path(path) {
            return decision(
                false,
                codes::UNSAFE_PATH,
                "path must be workspace-relative and must not traverse directories",
                RiskLevel::High,
            );
        }

        let scopes = match access {
            FileAccess::Read => &permissions.filesystem.read,
            FileAccess::Write => &permissions.filesystem.write,
        };
        if !scopes
            .iter()
            .filter(|scope| is_safe_scope(scope))
            .any(|scope| path_matches_scope(path, scope))
        {
            let (code, reason) = match access {
                FileAccess::Read => (
                    codes::READ_SCOPE,
                    "path is outside declared filesystem read scopes",
                ),
                FileAccess::Write => (
                    codes::WRITE_SCOPE,
                    "path is outside declared filesystem write scopes",
                ),
            };
            return decision(false, code, reason, RiskLevel::High);
        }

        decision(true, codes::ALLOWED, "allowed", RiskLevel::Low)
    }

    pub fn check_network(&self, host: &str, permissions: &Permissions) -> PolicyDecision {
        if !is_valid_host(host) {
            return decision(
                false,
                codes::INVALID_HOST,
                "network host is invalid",
                RiskLevel::High,
            );
        }

        if !permissions.network.enabled {
            return decision(
                false,
                codes::NETWORK_DISABLED,
                "network access is disabled by package permissions",
                RiskLevel::High,
            );
        }

        let host = host.to_ascii_lowercase();
        if !permissions.network.allow_hosts.iter().any(|allowed| {
            if !is_valid_host(allowed) {
                return false;
            }
            let allowed = allowed.to_ascii_lowercase();
            host == allowed || host.ends_with(&format!(".{allowed}"))
        }) {
            return decision(
                false,
                codes::HOST_NOT_ALLOWED,
                "network host is not in the declared allowlist",
                RiskLevel::High,
            );
        }

        decision(true, codes::ALLOWED, "allowed", RiskLevel::Medium)
    }

    pub fn check_environment(&self, name: &str, permissions: &Permissions) -> PolicyDecision {
        if !is_valid_environment_name(name) {
            return decision(
                false,
                codes::INVALID_ENVIRONMENT,
                "environment identifier is invalid",
                RiskLevel::High,
            );
        }

        if self.rules.identifies_secret(name) {
            return decision(
                false,
                codes::SECRET_IDENTIFIER,
                "environment identifier is classified as secret material",
                RiskLevel::Critical,
            );
        }

        if !permissions.secrets.read_environment {
            return decision(
                false,
                codes::ENVIRONMENT_DISABLED,
                "environment access is disabled by package permissions",
                RiskLevel::High,
            );
        }

        decision(true, codes::ALLOWED, "allowed", RiskLevel::Medium)
    }
}

fn decision(allowed: bool, code: &str, reason: &str, risk: RiskLevel) -> PolicyDecision {
    PolicyDecision {
        allowed,
        code: code.to_owned(),
        reason: reason.to_owned(),
        risk,
    }
}

fn command_name(program: &str) -> String {
    program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase()
}

fn command_line(program: &str, args: &[String]) -> String {
    let mut line = program.to_owned();
    for argument in args {
        line.push(' ');
        line.push_str(&argument.to_ascii_lowercase());
    }
    line
}

fn is_dangerous_command(program: &str, args: &[String]) -> bool {
    if [
        "sh",
        "bash",
        "dash",
        "zsh",
        "fish",
        "ksh",
        "pwsh",
        "powershell",
    ]
    .contains(&program)
        && args.iter().any(|argument| is_shell_command_flag(argument))
    {
        return true;
    }

    if program == "rm" {
        let mut recursive = false;
        let mut force = false;
        for argument in args {
            let argument = argument.to_ascii_lowercase();
            if argument == "--recursive" {
                recursive = true;
            } else if argument == "--force" {
                force = true;
            } else if argument.starts_with('-') && !argument.starts_with("--") {
                recursive |= argument.chars().any(|character| character == 'r');
                force |= argument.chars().any(|character| character == 'f');
            }
        }
        return recursive && force;
    }

    false
}

fn is_shell_command_flag(argument: &str) -> bool {
    let argument = argument.to_ascii_lowercase();
    if argument == "--command" || argument.starts_with("--command=") || argument == "/c" {
        return true;
    }
    argument
        .strip_prefix('-')
        .is_some_and(|flags| !flags.starts_with('-') && flags.contains('c'))
}

fn is_safe_workspace_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || has_windows_drive_prefix(path)
        || path.chars().any(|character| character.is_control())
    {
        return false;
    }

    !path.split(['/', '\\']).any(|segment| segment == "..")
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_safe_scope(scope: &str) -> bool {
    scope == "**" || is_safe_workspace_path(scope)
}

fn path_matches_scope(path: &str, scope: &str) -> bool {
    if scope == "**" {
        return true;
    }
    if let Some(prefix) = scope.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = scope.strip_suffix("/*") {
        return path.starts_with(&format!("{prefix}/")) && !path[prefix.len() + 1..].contains('/');
    }
    path == scope
}

fn is_valid_host(host: &str) -> bool {
    if host.is_empty()
        || host.trim() != host
        || host.len() > 253
        || host.contains(['/', '\\', ':', '?', '#'])
        || !host.is_ascii()
    {
        return false;
    }

    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn is_valid_environment_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('=')
        && !name
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}
