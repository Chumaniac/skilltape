//! Content-free build identity. This is not execution, Receipt or authorship proof.
use serde_json::{json, Value};

pub fn build_info(version: &str) -> Value {
    let catalog: Value = serde_json::from_str(include_str!("../../../product-capabilities.json"))
        .expect("the checked-in capability catalog is valid JSON");
    let source = env!("SKILLTAPE_BUILD_SOURCE");
    json!({
        "schema": "skilltape.build-info/v1", "version": version,
        "sourceRevision": if source.is_empty() { None } else { Some(source) },
        "dirty": env!("SKILLTAPE_BUILD_DIRTY") != "false",
        "channel": env!("SKILLTAPE_BUILD_CHANNEL"),
        "target": env!("SKILLTAPE_BUILD_TARGET"),
        "capabilities": catalog["capabilities"],
        "executionSupport": if cfg!(any(target_os="macos",target_os="linux")) { "native-sandbox-required" } else { "unsupported-fail-closed" }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_has_only_fixed_capabilities_and_no_workspace_authority() {
        let info = build_info("0.4.1");
        assert_eq!(info["schema"], "skilltape.build-info/v1");
        assert_eq!(info["version"], "0.4.1");
        assert_eq!(info["capabilities"].as_array().unwrap().len(), 5);
        assert!(
            info["sourceRevision"].is_null()
                || info["sourceRevision"].as_str().unwrap().len() == 40
        );
        assert!(info.get("workspace").is_none());
        assert!(info.get("canExecute").is_none());
    }
}
