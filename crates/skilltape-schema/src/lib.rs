pub mod model;
pub mod validation;

pub use model::{
    AssertStep, AssertionSpec, EntryPoint, ExecStep, FileStep, FilesystemPermissions, InputSpec,
    LockFile, NetworkPermissions, OutputSpec, Permissions, ProcessPermissions, ScriptStep,
    SecretPermissions, SkillManifest, Step, StepOutput, Workflow,
};
pub use validation::{validate_json, SchemaDiagnostic};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaId {
    SkillV1,
    WorkflowV1,
    PermissionsV1,
    LockV1,
}

impl SchemaId {
    pub const fn uri(self) -> &'static str {
        match self {
            Self::SkillV1 => "skilltape.dev/skill/v1",
            Self::WorkflowV1 => "skilltape.dev/workflow/v1",
            Self::PermissionsV1 => "skilltape.dev/permissions/v1",
            Self::LockV1 => "skilltape.dev/lock/v1",
        }
    }
}
