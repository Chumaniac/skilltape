use serde_json::json;
use skilltape_schema::{
    FilesystemPermissions, NetworkPermissions, Permissions, ProcessPermissions, SchemaId,
    SecretPermissions, Workflow,
};

use crate::steps::{compile_steps, StepCompilation};
use crate::{CompileError, CompileOutput, CompileRequest, Compiler, FixtureDraft};

const ENGINE_VERSION: &str = "0.1.0";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Provider-free compiler that translates capture metadata into a stable package draft.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicCompiler;

impl Compiler for DeterministicCompiler {
    fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompileError> {
        let StepCompilation {
            steps,
            provenance,
            read_scopes,
            write_scopes,
            executables,
            fixtures: step_fixtures,
        } = compile_steps(&request.tape)?;
        let workflow = Workflow {
            schema: SchemaId::WorkflowV1.uri().into(),
            steps,
        };
        let permissions = Permissions {
            schema: SchemaId::PermissionsV1.uri().into(),
            filesystem: FilesystemPermissions {
                read: read_scopes.into_iter().collect(),
                write: write_scopes.into_iter().collect(),
            },
            process: ProcessPermissions {
                executables: executables.iter().cloned().collect(),
                max_processes: 1,
                default_timeout_ms: DEFAULT_TIMEOUT_MS,
            },
            network: NetworkPermissions {
                enabled: false,
                allow_hosts: Vec::new(),
            },
            secrets: SecretPermissions {
                read_environment: false,
            },
        };
        let fixtures = package_support_files(&request, &executables, step_fixtures)?;
        let skill_markdown = render_skill_markdown(&request, workflow.steps.len());

        CompileOutput::try_new(
            &request.tape,
            workflow,
            permissions,
            skill_markdown,
            fixtures,
            provenance,
        )
    }
}

fn package_support_files(
    request: &CompileRequest,
    executables: &std::collections::BTreeSet<String>,
    step_fixtures: FixtureDraft,
) -> Result<FixtureDraft, CompileError> {
    let mut files = step_fixtures.files;
    files.insert("skilltape.yaml".into(), render_manifest(request));
    files.insert(
        "skilltape.lock".into(),
        render_lockfile(request, executables)?,
    );
    files.insert("README.md".into(), render_readme(request));
    Ok(FixtureDraft::new(files))
}

fn render_manifest(request: &CompileRequest) -> String {
    format!(
        "schema: {}\nname: {}\nversion: {}\ndescription: Deterministic workflow compiled from SkillTape metadata.\nengine:\n  min_version: {}\nentrypoint:\n  workflow: workflow.yaml\n  permissions: permissions.json\n  lockfile: skilltape.lock\ninputs: []\noutputs: []\ntargets:\n  - {}\n",
        SchemaId::SkillV1.uri(),
        quote_yaml_string(&request.name),
        quote_yaml_string(&request.target.version),
        quote_yaml_string(ENGINE_VERSION),
        quote_yaml_string(&request.target.name),
    )
}

fn render_lockfile(
    request: &CompileRequest,
    executables: &std::collections::BTreeSet<String>,
) -> Result<String, CompileError> {
    let tools = executables
        .iter()
        .map(|program| {
            json!({
                "program": program,
                "version": request.target.version,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::to_string_pretty(&json!({
        "schema": SchemaId::LockV1.uri(),
        "engine": {"version": ENGINE_VERSION},
        "tools": tools,
        "scripts": [],
    }))? + "\n")
}

fn render_skill_markdown(request: &CompileRequest, step_count: usize) -> String {
    format!(
        "# {}\n\nDeterministic SkillTape workflow for target `{}`.\n\nThis package contains {step_count} structured workflow step(s). Captured terminal output and environment values are not persisted.\n",
        request.name,
        request.target.identity(),
    )
}

fn render_readme(request: &CompileRequest) -> String {
    format!(
        "# {}\n\nThis package was compiled deterministically from SkillTape metadata for `{}`.\n",
        request.name,
        request.target.identity(),
    )
}

fn quote_yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}
