use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The kind of filesystem access a policy check is authorizing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAccess {
    Read,
    Write,
}

/// Deterministic, side-effect-free policy vocabulary shared by SkillTape
/// compilation, linting, and replay.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyRules {
    pub(crate) denied_programs: BTreeSet<String>,
    pub(crate) denied_argument_fragments: BTreeSet<String>,
    pub(crate) secret_identifiers: BTreeSet<String>,
}

impl Default for PolicyRules {
    fn default() -> Self {
        Self {
            denied_programs: [
                "chmod",
                "chown",
                "chgrp",
                "cmd",
                "cmd.exe",
                "command.com",
                "doas",
                "kill",
                "killall",
                "mkfs",
                "mount",
                "pkill",
                "poweroff",
                "powershell",
                "powershell.exe",
                "pwsh",
                "pwsh.exe",
                "reboot",
                "shutdown",
                "sudo",
                "su",
                "umount",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            denied_argument_fragments: ["dd if=", "mkfs.", "rm -rf", "rm -fr"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            secret_identifiers: BTreeSet::new(),
        }
    }
}

impl PolicyRules {
    /// Adds a program name that must always be rejected as dangerous.
    pub fn with_denied_program(mut self, program: impl Into<String>) -> Self {
        let program = program.into().trim().to_ascii_lowercase();
        if !program.is_empty() {
            self.denied_programs.insert(program);
        }
        self
    }

    /// Adds a case-insensitive fragment matched against the normalized command
    /// line. Fragments are intended for organization-specific deny rules.
    pub fn with_denied_argument_fragment(mut self, fragment: impl Into<String>) -> Self {
        let fragment = fragment.into().trim().to_ascii_lowercase();
        if !fragment.is_empty() {
            self.denied_argument_fragments.insert(fragment);
        }
        self
    }

    /// Adds an environment identifier that should be treated as secret
    /// material even when environment access is otherwise enabled.
    pub fn with_secret_identifier(mut self, identifier: impl Into<String>) -> Self {
        let identifier = normalize_identifier(&identifier.into());
        if !identifier.is_empty() {
            self.secret_identifiers.insert(identifier);
        }
        self
    }

    pub(crate) fn denies_program(&self, program: &str) -> bool {
        self.denied_programs.contains(program)
    }

    pub(crate) fn denies_argument_fragment(&self, command_line: &str) -> bool {
        self.denied_argument_fragments
            .iter()
            .any(|fragment| command_line.contains(fragment))
    }

    pub(crate) fn identifies_secret(&self, name: &str) -> bool {
        let normalized = normalize_identifier(name);
        self.secret_identifiers.contains(&normalized)
            || [
                "secret",
                "token",
                "password",
                "passwd",
                "api_key",
                "apikey",
                "access_key",
                "private_key",
                "credential",
                "cookie",
            ]
            .iter()
            .any(|marker| normalized.contains(marker))
    }
}

pub(crate) fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
