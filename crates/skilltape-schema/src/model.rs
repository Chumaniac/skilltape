#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SkillManifest {
    pub schema: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub engine: serde_json::Value,
    pub entrypoint: EntryPoint,
    pub inputs: Vec<InputSpec>,
    pub outputs: Vec<OutputSpec>,
    pub targets: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct EntryPoint {
    pub workflow: String,
    pub permissions: String,
    pub lockfile: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct InputSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub input_type: String,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct OutputSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub path: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Workflow {
    pub schema: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Step {
    Exec(ExecStep),
    Script(ScriptStep),
    File(FileStep),
    Assert(AssertStep),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExecStep {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub outputs: Vec<StepOutput>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StepOutput {
    pub path: String,
    #[serde(rename = "type")]
    pub output_type: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ScriptStep {
    pub id: String,
    pub path: String,
    pub args: Vec<String>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub outputs: Vec<StepOutput>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FileStep {
    pub id: String,
    pub operation: String,
    #[serde(rename = "from")]
    pub from_path: String,
    #[serde(rename = "to")]
    pub to_path: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct AssertStep {
    pub id: String,
    pub assertion: AssertionSpec,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct AssertionSpec {
    #[serde(rename = "type")]
    pub assertion_type: String,
    pub path: Option<String>,
    pub schema: Option<String>,
    pub hash: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Permissions {
    pub schema: String,
    pub filesystem: FilesystemPermissions,
    pub process: ProcessPermissions,
    pub network: NetworkPermissions,
    pub secrets: SecretPermissions,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FilesystemPermissions {
    pub read: Vec<String>,
    pub write: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ProcessPermissions {
    pub executables: Vec<String>,
    pub max_processes: u32,
    pub default_timeout_ms: u64,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct NetworkPermissions {
    pub enabled: bool,
    pub allow_hosts: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SecretPermissions {
    pub read_environment: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct LockFile {
    pub schema: String,
    pub engine: serde_json::Value,
    pub tools: Vec<serde_json::Value>,
    pub scripts: Vec<serde_json::Value>,
}

impl SkillManifest {
    pub fn has_expected_schema(&self) -> bool {
        self.schema == "skilltape.dev/skill/v1"
    }
}

impl Workflow {
    pub fn has_expected_schema(&self) -> bool {
        self.schema == "skilltape.dev/workflow/v1"
    }
}

impl Permissions {
    pub fn has_expected_schema(&self) -> bool {
        self.schema == "skilltape.dev/permissions/v1"
    }
}

impl LockFile {
    pub fn has_expected_schema(&self) -> bool {
        self.schema == "skilltape.dev/lock/v1"
    }
}
