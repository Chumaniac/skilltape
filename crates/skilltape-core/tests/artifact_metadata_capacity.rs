use serde_json::{json, Value};
use skilltape_core::ARTIFACT_MAX_ENTRIES;
use skilltape_schema::{validate_json, SchemaId};

fn step(action: &str) -> Value {
    if action == "script" {
        json!({"action":"script","id":"bounded","path":"scripts/run.sh","args":[],"timeout_ms":1})
    } else {
        json!({"action":"exec","id":"bounded","program":"printf","args":[],"timeout_ms":1})
    }
}

#[test]
fn workflow_array_capacity_matches_the_shared_artifact_ceiling() {
    let ceiling = ARTIFACT_MAX_ENTRIES as usize;
    for count in [ceiling, ceiling + 1] {
        let workflow =
            json!({"schema":"skilltape.dev/workflow/v1","steps":vec![step("exec");count]});
        assert_eq!(
            validate_json(SchemaId::WorkflowV1, &workflow).is_ok(),
            count == ceiling
        );
        for action in ["exec", "script"] {
            for field in ["args", "outputs"] {
                let mut value = step(action);
                value[field] = if field == "args" {
                    json!(vec!["CAPACITY_PRIVATE_SENTINEL"; count])
                } else {
                    json!(vec![json!({"path":"outputs/result","type":"file"}); count])
                };
                let document = json!({"schema":"skilltape.dev/workflow/v1","steps":[value]});
                let result = validate_json(SchemaId::WorkflowV1, &document);
                assert_eq!(
                    result.is_ok(),
                    count == ceiling,
                    "{action} {field} must use the shared ceiling"
                );
                if let Err(diagnostics) = result {
                    assert!(
                        diagnostics.iter().all(|item| item.message.len() < 256
                            && !item.message.contains("CAPACITY_PRIVATE_SENTINEL")),
                        "Capacity diagnostics must not echo large metadata or private arguments"
                    );
                }
            }
        }
    }
}
