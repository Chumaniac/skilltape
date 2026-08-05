use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use skilltape_schema::{
    validate_json, FileStep, LockFile, SchemaDiagnostic, SchemaId, SkillManifest, Step, StepOutput,
    Workflow,
};
use thiserror::Error;

use crate::{DiagnosticLevel, LintReport};

pub const REQUIRED_FILES: [&str; 6] = [
    "skilltape.yaml",
    "workflow.yaml",
    "permissions.json",
    "skilltape.lock",
    "SKILL.md",
    "README.md",
];

pub struct SkillPackage;

#[derive(Clone, Debug)]
pub struct LoadedSkillPackage {
    pub root: PathBuf,
    pub manifest: SkillManifest,
    pub workflow: Workflow,
    pub permissions: skilltape_schema::Permissions,
    pub lockfile: LockFile,
    raw_manifest: serde_json::Value,
    raw_workflow: serde_json::Value,
    raw_permissions: serde_json::Value,
    raw_lockfile: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("PKG001 missing required package file: {file}")]
    MissingRequiredFile { file: String },
    #[error("invalid package root: {source}")]
    InvalidPackageRoot {
        #[source]
        source: std::io::Error,
    },
    #[error("PKG003 invalid package file {file}: {source}")]
    InvalidFile {
        file: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("PKG007 unsafe package path escapes root: {file}")]
    UnsafePackagePath { file: String },
    #[error("required package path is not a complete file: {file}")]
    IncompleteRequiredFile { file: String },
}

impl SkillPackage {
    pub fn load(root: impl AsRef<Path>) -> Result<LoadedSkillPackage, PackageError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|source| PackageError::InvalidPackageRoot { source })?;

        for file in REQUIRED_FILES {
            verify_required_file(&root, file)?;
        }

        let raw_manifest = read_yaml_json(&root, "skilltape.yaml")?;
        let raw_workflow = read_yaml_json(&root, "workflow.yaml")?;
        let raw_permissions = read_json(&root, "permissions.json")?;
        let raw_lockfile = read_json(&root, "skilltape.lock")?;

        let manifest = deserialize("skilltape.yaml", raw_manifest.clone())?;
        let workflow = deserialize("workflow.yaml", raw_workflow.clone())?;
        let permissions = deserialize("permissions.json", raw_permissions.clone())?;
        let lockfile = deserialize("skilltape.lock", raw_lockfile.clone())?;

        Ok(LoadedSkillPackage {
            root,
            manifest,
            workflow,
            permissions,
            lockfile,
            raw_manifest,
            raw_workflow,
            raw_permissions,
            raw_lockfile,
        })
    }
}

impl LoadedSkillPackage {
    pub fn lint(&self, strict: bool) -> LintReport {
        let mut report = LintReport {
            files_checked: REQUIRED_FILES.len(),
            ..LintReport::default()
        };

        self.lint_required_files(&mut report);
        self.lint_schema(&mut report);
        self.lint_entrypoints(&mut report);
        self.lint_workflow_permissions_and_paths(&mut report);
        self.lint_lockfile(&mut report, strict);

        report
    }

    fn lint_required_files(&self, report: &mut LintReport) {
        for file in REQUIRED_FILES {
            if !self.root.join(file).exists() {
                report.push(
                    "PKG001",
                    DiagnosticLevel::Error,
                    file,
                    "",
                    "required package file is missing",
                );
            }
        }
    }

    fn lint_schema(&self, report: &mut LintReport) {
        validate_schema(
            report,
            "skilltape.yaml",
            SchemaId::SkillV1,
            &self.raw_manifest,
        );
        validate_schema(
            report,
            "workflow.yaml",
            SchemaId::WorkflowV1,
            &self.raw_workflow,
        );
        validate_schema(
            report,
            "permissions.json",
            SchemaId::PermissionsV1,
            &self.raw_permissions,
        );
        validate_schema(
            report,
            "skilltape.lock",
            SchemaId::LockV1,
            &self.raw_lockfile,
        );
    }

    fn lint_entrypoints(&self, report: &mut LintReport) {
        let expected = [
            (
                "workflow",
                &self.manifest.entrypoint.workflow,
                "workflow.yaml",
            ),
            (
                "permissions",
                &self.manifest.entrypoint.permissions,
                "permissions.json",
            ),
            (
                "lockfile",
                &self.manifest.entrypoint.lockfile,
                "skilltape.lock",
            ),
        ];

        for (field, actual, expected) in expected {
            if actual != expected {
                report.push(
                    "PKG002",
                    DiagnosticLevel::Error,
                    "skilltape.yaml",
                    format!("entrypoint.{field}"),
                    format!("entrypoint {field} must be {expected}"),
                );
            }

            if is_unsafe_workspace_path(actual) {
                report.push(
                    "PKG007",
                    DiagnosticLevel::Error,
                    "skilltape.yaml",
                    format!("entrypoint.{field}"),
                    "entrypoint path must be relative and must not traverse directories",
                );
            }
        }
    }

    fn lint_workflow_permissions_and_paths(&self, report: &mut LintReport) {
        let input_ids = self
            .manifest
            .inputs
            .iter()
            .map(|input| input.id.as_str())
            .collect::<BTreeSet<_>>();
        let manifest_output_paths = self
            .manifest
            .outputs
            .iter()
            .map(|output| output.path.as_str())
            .collect::<BTreeSet<_>>();

        for (index, step) in self.workflow.steps.iter().enumerate() {
            match step {
                Step::Exec(step) => {
                    if !self
                        .permissions
                        .process
                        .executables
                        .iter()
                        .any(|program| program == &step.program)
                    {
                        report.push(
                            "PKG004",
                            DiagnosticLevel::Error,
                            "workflow.yaml",
                            format!("steps[{index}].program"),
                            format!(
                                "executable `{}` is not declared in permissions",
                                step.program
                            ),
                        );
                    }

                    lint_args(report, index, &step.args, &input_ids);
                    lint_step_outputs(
                        report,
                        index,
                        &step.outputs,
                        &self.permissions.filesystem.write,
                        &manifest_output_paths,
                    );
                }
                Step::Script(step) => {
                    lint_read_path(
                        report,
                        &step.path,
                        "workflow.yaml",
                        format!("steps[{index}].path"),
                        &self.permissions.filesystem.read,
                    );
                    lint_args(report, index, &step.args, &input_ids);
                    lint_step_outputs(
                        report,
                        index,
                        &step.outputs,
                        &self.permissions.filesystem.write,
                        &manifest_output_paths,
                    );
                }
                Step::File(step) => {
                    lint_file_step(report, index, step, &self.permissions.filesystem.read);
                    lint_write_path(
                        report,
                        &step.to_path,
                        "workflow.yaml",
                        format!("steps[{index}].to"),
                        &self.permissions.filesystem.write,
                    );
                }
                Step::Assert(step) => {
                    if let Some(path) = &step.assertion.path {
                        lint_read_path(
                            report,
                            path,
                            "workflow.yaml",
                            format!("steps[{index}].assertion.path"),
                            &self.permissions.filesystem.read,
                        );
                    }
                }
            }
        }
    }

    fn lint_lockfile(&self, report: &mut LintReport, strict: bool) {
        let manifest_engine = self
            .manifest
            .engine
            .get("min_version")
            .and_then(serde_json::Value::as_str);
        let lock_engine = self
            .lockfile
            .engine
            .get("version")
            .and_then(serde_json::Value::as_str);

        if manifest_engine.is_some() && lock_engine.is_some() && manifest_engine != lock_engine {
            report.push(
                "PKG010",
                if strict {
                    DiagnosticLevel::Error
                } else {
                    DiagnosticLevel::Warning
                },
                "skilltape.lock",
                "engine.version",
                "lockfile engine version does not match the manifest minimum engine version",
            );
        }

        let locked_tools = self
            .lockfile
            .tools
            .iter()
            .filter_map(|tool| tool.get("program").and_then(serde_json::Value::as_str))
            .collect::<BTreeSet<_>>();

        for step in &self.workflow.steps {
            if let Step::Exec(step) = step {
                if !locked_tools.contains(step.program.as_str()) {
                    report.push(
                        "PKG010",
                        DiagnosticLevel::Error,
                        "skilltape.lock",
                        "tools",
                        format!("executable `{}` is missing from the lockfile", step.program),
                    );
                }
            }
        }

        let locked_scripts = self
            .lockfile
            .scripts
            .iter()
            .filter_map(|script| script.get("path").and_then(serde_json::Value::as_str))
            .collect::<BTreeSet<_>>();

        for step in &self.workflow.steps {
            if let Step::Script(step) = step {
                if !locked_scripts.contains(step.path.as_str()) {
                    report.push(
                        "PKG010",
                        DiagnosticLevel::Error,
                        "skilltape.lock",
                        "scripts",
                        format!("script `{}` is missing from the lockfile", step.path),
                    );
                }
            }
        }
    }
}

fn verify_required_file(root: &Path, file: &str) -> Result<(), PackageError> {
    let path = root.join(file);

    if !path.exists() {
        return Err(PackageError::MissingRequiredFile {
            file: file.to_owned(),
        });
    }

    let canonical = path
        .canonicalize()
        .map_err(|_| PackageError::UnsafePackagePath {
            file: file.to_owned(),
        })?;
    if !canonical.starts_with(root) {
        return Err(PackageError::UnsafePackagePath {
            file: file.to_owned(),
        });
    }

    let metadata = fs::metadata(&path).map_err(|_| PackageError::IncompleteRequiredFile {
        file: file.to_owned(),
    })?;
    if !metadata.is_file() {
        return Err(PackageError::IncompleteRequiredFile {
            file: file.to_owned(),
        });
    }

    Ok(())
}

fn read_yaml_json(root: &Path, file: &str) -> Result<serde_json::Value, PackageError> {
    let contents = read_to_string(root, file)?;
    serde_yaml::from_str(&contents).map_err(|source| PackageError::InvalidFile {
        file: file.to_owned(),
        source: Box::new(source),
    })
}

fn read_json(root: &Path, file: &str) -> Result<serde_json::Value, PackageError> {
    let contents = read_to_string(root, file)?;
    serde_json::from_str(&contents).map_err(|source| PackageError::InvalidFile {
        file: file.to_owned(),
        source: Box::new(source),
    })
}

fn read_to_string(root: &Path, file: &str) -> Result<String, PackageError> {
    fs::read_to_string(root.join(file)).map_err(|source| PackageError::InvalidFile {
        file: file.to_owned(),
        source: Box::new(source),
    })
}

fn deserialize<T>(file: &str, value: serde_json::Value) -> Result<T, PackageError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value).map_err(|source| PackageError::InvalidFile {
        file: file.to_owned(),
        source: Box::new(source),
    })
}

fn validate_schema(
    report: &mut LintReport,
    file: &str,
    schema_id: SchemaId,
    value: &serde_json::Value,
) {
    if let Err(diagnostics) = validate_json(schema_id, value) {
        for diagnostic in diagnostics {
            report.push(
                "PKG003",
                DiagnosticLevel::Error,
                file,
                schema_path(&diagnostic),
                diagnostic.message,
            );
        }
    }
}

fn schema_path(diagnostic: &SchemaDiagnostic) -> String {
    let base = json_pointer_to_path(&diagnostic.instance_path);
    if diagnostic.keyword == "additionalProperties" {
        if let Some(property) = unexpected_property(&diagnostic.message) {
            return if base.is_empty() {
                property
            } else {
                format!("{base}.{property}")
            };
        }
    }
    base
}

fn unexpected_property(message: &str) -> Option<String> {
    let mut chars = message.char_indices();
    while let Some((_, ch)) = chars.next() {
        if ch == '\'' || ch == '"' || ch == '`' {
            let quote = ch;
            let start = chars.next()?.0;
            for (end, candidate) in chars.by_ref() {
                if candidate == quote {
                    return Some(message[start..end].to_owned());
                }
            }
            return None;
        }
    }
    None
}

fn json_pointer_to_path(pointer: &str) -> String {
    let pointer = pointer.strip_prefix('/').unwrap_or(pointer);
    if pointer.is_empty() {
        return String::new();
    }

    let mut path = String::new();
    for segment in pointer.split('/') {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        if segment.chars().all(|ch| ch.is_ascii_digit()) {
            path.push('[');
            path.push_str(&segment);
            path.push(']');
        } else {
            if !path.is_empty() {
                path.push('.');
            }
            path.push_str(&segment);
        }
    }
    path
}

fn lint_args(
    report: &mut LintReport,
    step_index: usize,
    args: &[String],
    declared_inputs: &BTreeSet<&str>,
) {
    for (arg_index, arg) in args.iter().enumerate() {
        for input in input_references(arg) {
            if !declared_inputs.contains(input.as_str()) {
                report.push(
                    "PKG008",
                    DiagnosticLevel::Error,
                    "workflow.yaml",
                    format!("steps[{step_index}].args[{arg_index}]"),
                    format!("input `{input}` is not declared in the manifest"),
                );
            }
        }
    }
}

fn input_references(value: &str) -> Vec<String> {
    let mut inputs = Vec::new();
    let mut rest = value;

    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("}}") else {
            break;
        };
        let expression = rest[..end].trim();
        if let Some(input) = expression.strip_prefix("inputs.") {
            let input = input
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
                .next()
                .unwrap_or_default();
            if !input.is_empty() {
                inputs.push(input.to_owned());
            }
        }
        rest = &rest[end + 2..];
    }

    inputs
}

fn lint_file_step(report: &mut LintReport, index: usize, step: &FileStep, read_scopes: &[String]) {
    lint_read_path(
        report,
        &step.from_path,
        "workflow.yaml",
        format!("steps[{index}].from"),
        read_scopes,
    );
}

fn lint_step_outputs(
    report: &mut LintReport,
    step_index: usize,
    outputs: &[StepOutput],
    write_scopes: &[String],
    manifest_output_paths: &BTreeSet<&str>,
) {
    for (output_index, output) in outputs.iter().enumerate() {
        let path = format!("steps[{step_index}].outputs[{output_index}].path");
        lint_write_path(
            report,
            &output.path,
            "workflow.yaml",
            path.clone(),
            write_scopes,
        );
        if !manifest_output_paths.contains(output.path.as_str()) {
            report.push(
                "PKG009",
                DiagnosticLevel::Error,
                "workflow.yaml",
                path,
                format!(
                    "workflow output `{}` is not declared by the manifest",
                    output.path
                ),
            );
        }
    }
}

fn lint_read_path(
    report: &mut LintReport,
    value: &str,
    file: &str,
    path: String,
    scopes: &[String],
) {
    lint_workspace_path_safety(report, value, file, path.clone());
    if !path_matches_any_scope(value, scopes) {
        report.push(
            "PKG005",
            DiagnosticLevel::Error,
            file,
            path,
            format!("path `{value}` is outside declared filesystem read scopes"),
        );
    }
}

fn lint_write_path(
    report: &mut LintReport,
    value: &str,
    file: &str,
    path: String,
    scopes: &[String],
) {
    lint_workspace_path_safety(report, value, file, path.clone());
    if !path_matches_any_scope(value, scopes) {
        report.push(
            "PKG006",
            DiagnosticLevel::Error,
            file,
            path,
            format!("path `{value}` is outside declared filesystem write scopes"),
        );
    }
}

fn lint_workspace_path_safety(report: &mut LintReport, value: &str, file: &str, path: String) {
    if is_unsafe_workspace_path(value) {
        report.push(
            "PKG007",
            DiagnosticLevel::Error,
            file,
            path,
            format!("path `{value}` must be relative and must not traverse directories"),
        );
    }
}

fn is_unsafe_workspace_path(value: &str) -> bool {
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || has_windows_drive_prefix(value)
    {
        return true;
    }

    Path::new(value).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) || value.split(['/', '\\']).any(|segment| segment == "..")
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

fn path_matches_any_scope(path: &str, scopes: &[String]) -> bool {
    scopes.iter().any(|scope| path_matches_scope(path, scope))
}

fn path_matches_scope(path: &str, scope: &str) -> bool {
    if let Some(prefix) = scope.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }

    if let Some(prefix) = scope.strip_suffix("/*") {
        return path.starts_with(&format!("{prefix}/")) && !path[prefix.len() + 1..].contains('/');
    }

    path == scope
}
