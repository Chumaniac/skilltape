use crate::SchemaId;

const SKILL_V1_SCHEMA: &str = include_str!("../../../schemas/skill/v1.json");
const WORKFLOW_V1_SCHEMA: &str = include_str!("../../../schemas/workflow/v1.json");
const PERMISSIONS_V1_SCHEMA: &str = include_str!("../../../schemas/permissions/v1.json");
const LOCK_V1_SCHEMA: &str = include_str!("../../../schemas/lock/v1.json");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaDiagnostic {
    pub instance_path: String,
    pub keyword: String,
    pub message: String,
}

impl SchemaDiagnostic {
    fn from_schema_error(error: jsonschema::ValidationError<'_>) -> Self {
        Self::from_error(error)
    }

    fn from_validation_error(error: jsonschema::ValidationError<'_>) -> Self {
        Self::from_error(error)
    }

    fn from_error(error: jsonschema::ValidationError<'_>) -> Self {
        let schema_path = error.schema_path().as_str();
        let keyword = schema_path
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("schema")
            .to_owned();

        Self {
            instance_path: error.instance_path().as_str().to_owned(),
            keyword,
            message: error.to_string(),
        }
    }
}

pub fn validate_json(
    schema_id: SchemaId,
    document: &serde_json::Value,
) -> Result<(), Vec<SchemaDiagnostic>> {
    let schema = schema_value(schema_id);
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| vec![SchemaDiagnostic::from_schema_error(error)])?;
    let errors = validator
        .iter_errors(document)
        .map(SchemaDiagnostic::from_validation_error)
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn schema_value(schema_id: SchemaId) -> serde_json::Value {
    let schema = match schema_id {
        SchemaId::SkillV1 => SKILL_V1_SCHEMA,
        SchemaId::WorkflowV1 => WORKFLOW_V1_SCHEMA,
        SchemaId::PermissionsV1 => PERMISSIONS_V1_SCHEMA,
        SchemaId::LockV1 => LOCK_V1_SCHEMA,
    };

    serde_json::from_str(schema).expect("embedded JSON Schema must be valid JSON")
}
