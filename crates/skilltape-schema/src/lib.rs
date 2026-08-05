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
