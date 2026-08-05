use skilltape_schema::{validate_json, SchemaId};

#[test]
fn rejects_workflow_with_unknown_action() {
    let document = serde_json::json!({
        "schema": "skilltape.dev/workflow/v1",
        "steps": [{
            "id": "unsafe",
            "action": "shell",
            "program": "sh",
            "args": [],
            "timeout_ms": 1000
        }]
    });

    let errors = validate_json(SchemaId::WorkflowV1, &document).expect_err("must reject");

    assert!(errors.iter().any(|error| error.keyword == "enum"));
}

#[test]
fn rejects_absolute_output_path() {
    let document = serde_json::json!({
        "schema": "skilltape.dev/workflow/v1",
        "steps": [{
            "id": "write",
            "action": "file",
            "operation": "copy",
            "from": "input.txt",
            "to": "/tmp/output.txt"
        }]
    });

    let errors = validate_json(SchemaId::WorkflowV1, &document).expect_err("must reject");

    assert!(errors.iter().any(|error| error.keyword == "pattern"));
}

#[test]
fn accepts_minimal_skill_manifest() {
    let document = serde_json::json!({
        "schema": "skilltape.dev/skill/v1",
        "name": "minimal-skill",
        "version": "0.1.0",
        "description": "A minimal SkillTape package.",
        "engine": {"min_version": "0.1.0"},
        "entrypoint": {
            "workflow": "workflow.yaml",
            "permissions": "permissions.json",
            "lockfile": "skilltape.lock"
        },
        "inputs": [],
        "outputs": [],
        "targets": ["generic-agent-skill"]
    });

    validate_json(SchemaId::SkillV1, &document).expect("manifest should validate");
}
