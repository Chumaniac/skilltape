use skilltape_schema::model::{Step, Workflow};
use skilltape_schema::{
    AssertStep, AssertionSpec, EntryPoint, ExecStep, FileStep, FilesystemPermissions, InputSpec,
    LockFile, NetworkPermissions, OutputSpec, Permissions, ProcessPermissions, ScriptStep,
    SecretPermissions, SkillManifest, StepOutput,
};

#[test]
fn parses_exec_step_from_yaml() {
    let yaml = r#"
schema: skilltape.dev/workflow/v1
steps:
  - id: extract-text
    action: exec
    program: pdftotext
    args:
      - "{{ inputs.source_pdf }}"
      - "work/input.txt"
    timeout_ms: 60000
"#;

    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow should parse");
    assert_eq!(workflow.steps.len(), 1);
    match &workflow.steps[0] {
        Step::Exec(step) => {
            assert_eq!(step.id, "extract-text");
            assert_eq!(step.program, "pdftotext");
            assert_eq!(step.args.len(), 2);
        }
        other => panic!("expected exec step, got {other:?}"),
    }
}

#[test]
fn exposes_the_complete_typed_model_api() {
    #[allow(clippy::too_many_arguments)]
    fn accepts_model_types(
        _: Option<SkillManifest>,
        _: Option<EntryPoint>,
        _: Option<InputSpec>,
        _: Option<OutputSpec>,
        _: Option<ExecStep>,
        _: Option<ScriptStep>,
        _: Option<FileStep>,
        _: Option<AssertStep>,
        _: Option<AssertionSpec>,
        _: Option<StepOutput>,
        _: Option<Permissions>,
        _: Option<FilesystemPermissions>,
        _: Option<ProcessPermissions>,
        _: Option<NetworkPermissions>,
        _: Option<SecretPermissions>,
        _: Option<LockFile>,
    ) {
    }

    accepts_model_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None,
    );
}
