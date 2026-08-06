use skilltape_policy::{FileAccess, PolicyDecision, PolicyEngine, PolicyRules, RiskLevel};
use skilltape_schema::{
    FilesystemPermissions, NetworkPermissions, Permissions, ProcessPermissions, SecretPermissions,
};

fn permissions(
    read: &[&str],
    write: &[&str],
    executables: &[&str],
    network_enabled: bool,
    allow_hosts: &[&str],
    read_environment: bool,
) -> Permissions {
    Permissions {
        schema: "skilltape.dev/permissions/v1".to_owned(),
        filesystem: FilesystemPermissions {
            read: read.iter().map(|value| (*value).to_owned()).collect(),
            write: write.iter().map(|value| (*value).to_owned()).collect(),
        },
        process: ProcessPermissions {
            executables: executables
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            max_processes: 1,
            default_timeout_ms: 30_000,
        },
        network: NetworkPermissions {
            enabled: network_enabled,
            allow_hosts: allow_hosts
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        secrets: SecretPermissions { read_environment },
    }
}

#[test]
fn allows_declared_safe_commands_and_denies_undeclared_executables() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&[], &[], &["printf"], false, &[], false);

    let allowed = engine.check_command("printf", &["hello".to_owned()], &permissions);
    assert_decision(&allowed, true, "POLICY_ALLOWED", RiskLevel::Low);

    let denied = engine.check_command("python", &[], &permissions);
    assert_decision(&denied, false, "PKG004", RiskLevel::High);
}

#[test]
fn denies_dangerous_commands_even_when_declared() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&[], &[], &["rm", "sh"], false, &[], false);

    let recursive_delete =
        engine.check_command("rm", &["-rf".to_owned(), "output".to_owned()], &permissions);
    assert_decision(
        &recursive_delete,
        false,
        "POLICY_COMMAND_DANGEROUS",
        RiskLevel::Critical,
    );

    let shell = engine.check_command(
        "sh",
        &["-c".to_owned(), "echo hello".to_owned()],
        &permissions,
    );
    assert_decision(
        &shell,
        false,
        "POLICY_COMMAND_DANGEROUS",
        RiskLevel::Critical,
    );
}

#[test]
fn denies_windows_shell_interpreters_even_when_declared() {
    let engine = PolicyEngine::default();
    let permissions = permissions(
        &[],
        &[],
        &[
            "cmd.exe",
            "command.com",
            r"C:\Windows\System32\CMD.EXE",
            "C:/Windows/command.com",
        ],
        false,
        &[],
        false,
    );

    for (program, args) in [
        ("cmd.exe", vec!["/c".to_owned(), "echo hello".to_owned()]),
        ("cmd.exe", vec!["/k".to_owned(), "echo hello".to_owned()]),
        (
            r"C:\Windows\System32\CMD.EXE",
            vec!["/c".to_owned(), "echo hello".to_owned()],
        ),
        (
            "command.com",
            vec!["/c".to_owned(), "echo hello".to_owned()],
        ),
        (
            "C:/Windows/command.com",
            vec!["/c".to_owned(), "echo hello".to_owned()],
        ),
    ] {
        let decision = engine.check_command(program, &args, &permissions);
        assert_decision(
            &decision,
            false,
            "POLICY_COMMAND_DANGEROUS",
            RiskLevel::Critical,
        );
    }
}

#[test]
fn denies_declared_windows_shell_aliases_and_path_variants() {
    let engine = PolicyEngine::default();
    let cases = [
        ("cmd", vec!["/c".to_owned(), "echo hello".to_owned()]),
        ("cmd.exe", vec!["/k".to_owned(), "echo hello".to_owned()]),
        (
            r"C:\Windows\System32\CMD.EXE",
            vec!["/c".to_owned(), "echo hello".to_owned()],
        ),
        (
            "CoMmAnD.CoM",
            vec!["/c".to_owned(), "echo hello".to_owned()],
        ),
        (
            "C:/Windows/System32/command.com",
            vec!["/k".to_owned(), "echo hello".to_owned()],
        ),
        (
            "powershell",
            vec!["-Command".to_owned(), "Write-Output hello".to_owned()],
        ),
        (
            "PowerShell.EXE",
            vec!["-Command".to_owned(), "Write-Output hello".to_owned()],
        ),
        (
            r"C:\Windows\System32\PowerShell.EXE",
            vec!["-Command".to_owned(), "Write-Output hello".to_owned()],
        ),
        (
            "pwsh",
            vec!["-Command".to_owned(), "Write-Output hello".to_owned()],
        ),
        (
            "PwSh.ExE",
            vec!["-Command".to_owned(), "Write-Output hello".to_owned()],
        ),
        (
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            vec!["-Command".to_owned(), "Write-Output hello".to_owned()],
        ),
    ];
    let executables = cases
        .iter()
        .map(|(program, _)| (*program).to_owned())
        .collect::<Vec<_>>();
    let permissions = permissions(
        &[],
        &[],
        &executables.iter().map(String::as_str).collect::<Vec<_>>(),
        false,
        &[],
        false,
    );

    for (program, args) in cases {
        let decision = engine.check_command(program, &args, &permissions);
        assert_decision(
            &decision,
            false,
            "POLICY_COMMAND_DANGEROUS",
            RiskLevel::Critical,
        );
    }
}

#[test]
fn preserves_dangerous_command_detection_without_rejecting_safe_text() {
    let engine = PolicyEngine::default();
    let permissions = permissions(
        &[],
        &[],
        &[
            "printf",
            "rm",
            "dd",
            "mkfs.ext4",
            "shutdown",
            "reboot",
            "poweroff",
        ],
        false,
        &[],
        false,
    );

    let safe_text = engine.check_command("printf", &["shutdown".to_owned()], &permissions);
    assert_decision(&safe_text, true, "POLICY_ALLOWED", RiskLevel::Low);

    for (program, args) in [
        ("shutdown", vec!["now".to_owned()]),
        ("reboot", vec!["now".to_owned()]),
        ("poweroff", Vec::new()),
        ("rm", vec!["-rf".to_owned(), "output".to_owned()]),
        ("rm", vec!["-fr".to_owned(), "output".to_owned()]),
        (
            "dd",
            vec!["if=/dev/zero".to_owned(), "of=output".to_owned()],
        ),
        ("mkfs.ext4", vec!["disk.img".to_owned()]),
    ] {
        let decision = engine.check_command(program, &args, &permissions);
        assert_decision(
            &decision,
            false,
            "POLICY_COMMAND_DANGEROUS",
            RiskLevel::Critical,
        );
    }
}

#[test]
fn built_in_dangerous_rules_are_contextual_not_argument_substrings() {
    let engine = PolicyEngine::default();
    let permissions = permissions(
        &[],
        &[],
        &["printf", "rm", "dd", "mkfs", "mkfs.ext4"],
        false,
        &[],
        false,
    );

    for text in ["rm -rf", "dd if=/dev/zero", "mkfs.ext4"] {
        let decision = engine.check_command("printf", &[text.to_owned()], &permissions);
        assert_decision(&decision, true, "POLICY_ALLOWED", RiskLevel::Low);
    }

    let dd_without_input = engine.check_command("dd", &["of=output".to_owned()], &permissions);
    assert_decision(&dd_without_input, true, "POLICY_ALLOWED", RiskLevel::Low);

    for (program, args) in [
        ("rm", vec!["-rf".to_owned(), "output".to_owned()]),
        (
            "dd",
            vec!["if=/dev/zero".to_owned(), "of=output".to_owned()],
        ),
        ("mkfs", vec!["disk.img".to_owned()]),
        ("mkfs.ext4", vec!["disk.img".to_owned()]),
    ] {
        let decision = engine.check_command(program, &args, &permissions);
        assert_decision(
            &decision,
            false,
            "POLICY_COMMAND_DANGEROUS",
            RiskLevel::Critical,
        );
    }
}

#[test]
fn path_checks_use_workspace_relative_scope_boundaries() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&["inputs/**"], &["outputs/**"], &[], false, &[], false);

    let read = engine.check_path("inputs/source.txt", FileAccess::Read, &permissions);
    assert_decision(&read, true, "POLICY_ALLOWED", RiskLevel::Low);

    let boundary = engine.check_path("inputs-other/source.txt", FileAccess::Read, &permissions);
    assert_decision(&boundary, false, "PKG005", RiskLevel::High);

    let wrong_access = engine.check_path("inputs/source.txt", FileAccess::Write, &permissions);
    assert_decision(&wrong_access, false, "PKG006", RiskLevel::High);
}

#[test]
fn path_checks_reject_windows_separator_scope_bypass() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&["foo/*"], &[], &[], false, &[], false);

    let nested_windows_path =
        engine.check_path(r"foo/bar\secret.txt", FileAccess::Read, &permissions);
    assert_decision(&nested_windows_path, false, "PKG007", RiskLevel::High);
}

#[test]
fn bare_double_star_does_not_grant_workspace_wide_access() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&["**"], &[], &[], false, &[], false);

    let decision = engine.check_path("workspace/file.txt", FileAccess::Read, &permissions);
    assert_decision(&decision, false, "PKG005", RiskLevel::High);
}

#[test]
fn path_checks_reject_absolute_drive_and_traversal_forms() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&["inputs/**"], &["outputs/**"], &[], false, &[], false);

    for unsafe_path in [
        "",
        "/tmp/file",
        "\\\\server\\share\\file",
        "C:\\tmp\\file",
        "folder/../secret",
        "folder\\..\\secret",
    ] {
        let decision = engine.check_path(unsafe_path, FileAccess::Read, &permissions);
        assert_decision(&decision, false, "PKG007", RiskLevel::High);
    }
}

#[test]
fn network_requires_enablement_and_exact_or_subdomain_allowlist() {
    let engine = PolicyEngine::default();
    let disabled = permissions(&[], &[], &[], false, &[], false);
    let decision = engine.check_network("api.example.com", &disabled);
    assert_decision(&decision, false, "POLICY_NETWORK_DISABLED", RiskLevel::High);

    let enabled = permissions(&[], &[], &[], true, &["example.com", "api.internal"], false);
    let exact = engine.check_network("example.com", &enabled);
    assert_decision(&exact, true, "POLICY_ALLOWED", RiskLevel::Medium);

    let subdomain = engine.check_network("v1.api.internal", &enabled);
    assert_decision(&subdomain, true, "POLICY_ALLOWED", RiskLevel::Medium);

    let boundary = engine.check_network("notexample.com", &enabled);
    assert_decision(
        &boundary,
        false,
        "POLICY_NETWORK_HOST_NOT_ALLOWED",
        RiskLevel::High,
    );
}

#[test]
fn network_rejects_malformed_hosts_without_echoing_them() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&[], &[], &[], true, &["example.com"], false);

    for host in ["", "https://example.com", "example.com:443", "bad host"] {
        let decision = engine.check_network(host, &permissions);
        assert_decision(
            &decision,
            false,
            "POLICY_NETWORK_INVALID_HOST",
            RiskLevel::High,
        );
        if !host.is_empty() {
            assert!(!decision.reason.contains(host));
        }
    }
}

#[test]
fn environment_is_denied_by_default_and_secret_names_stay_denied() {
    let engine = PolicyEngine::default();
    let disabled = permissions(&[], &[], &[], false, &[], false);
    let disabled_decision = engine.check_environment("LANG", &disabled);
    assert_decision(
        &disabled_decision,
        false,
        "POLICY_ENVIRONMENT_DISABLED",
        RiskLevel::High,
    );

    let enabled = permissions(&[], &[], &[], false, &[], true);
    let ordinary = engine.check_environment("LANG", &enabled);
    assert_decision(&ordinary, true, "POLICY_ALLOWED", RiskLevel::Medium);

    let secret = engine.check_environment("AWS_SECRET_ACCESS_KEY", &enabled);
    assert_decision(
        &secret,
        false,
        "POLICY_SECRET_IDENTIFIER",
        RiskLevel::Critical,
    );
    assert!(!secret.reason.contains("AWS_SECRET_ACCESS_KEY"));
}

#[test]
fn environment_rejects_empty_names_and_never_echoes_secret_inputs() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&[], &[], &[], false, &[], true);

    let empty = engine.check_environment("", &permissions);
    assert_decision(&empty, false, "POLICY_ENVIRONMENT_INVALID", RiskLevel::High);

    let secret = engine.check_environment("DB_PASSWORD", &permissions);
    assert_decision(
        &secret,
        false,
        "POLICY_SECRET_IDENTIFIER",
        RiskLevel::Critical,
    );
    assert!(!secret.reason.contains("DB_PASSWORD"));
}

#[test]
fn decisions_are_serializable_comparable_and_stable() {
    let engine = PolicyEngine::default();
    let permissions = permissions(&[], &[], &["printf"], false, &[], false);
    let first = engine.check_command("printf", &[], &permissions);
    let second = engine.check_command("printf", &[], &permissions);
    assert_eq!(first, second);
    assert_eq!(first, first.clone());

    let encoded = serde_json::to_string(&first).expect("decision JSON");
    assert_eq!(
        encoded,
        r#"{"allowed":true,"code":"POLICY_ALLOWED","reason":"allowed","risk":"low"}"#
    );
}

#[test]
fn custom_rules_extend_danger_and_secret_vocabulary_deterministically() {
    let rules = PolicyRules::default()
        .with_denied_program("internal-tool")
        .with_denied_argument_fragment("--unsafe-mode")
        .with_secret_identifier("private_material");
    let engine = PolicyEngine::new(rules);
    let permissions = permissions(&[], &[], &["internal-tool", "printf"], false, &[], true);

    let program = engine.check_command("internal-tool", &[], &permissions);
    assert_decision(
        &program,
        false,
        "POLICY_COMMAND_DANGEROUS",
        RiskLevel::Critical,
    );

    let argument = engine.check_command("printf", &["--unsafe-mode".to_owned()], &permissions);
    assert_decision(
        &argument,
        false,
        "POLICY_COMMAND_DANGEROUS",
        RiskLevel::Critical,
    );

    let secret = engine.check_environment("PRIVATE_MATERIAL", &permissions);
    assert_decision(
        &secret,
        false,
        "POLICY_SECRET_IDENTIFIER",
        RiskLevel::Critical,
    );
}

fn assert_decision(decision: &PolicyDecision, allowed: bool, code: &str, risk: RiskLevel) {
    assert_eq!(decision.allowed, allowed);
    assert_eq!(decision.code, code);
    assert_eq!(decision.risk, risk);
    assert!(!decision.reason.is_empty());
}
