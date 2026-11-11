//! Optional bounded association with a caller-provided successful Receipt.
use std::fmt;
use std::path::Path;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest, Sha256};
use skilltape_core::LoadedSkillPackage;
use skilltape_runner::publish_directory_noreplace;
use skilltape_schema::{validate_json, SchemaId};
use tempfile::Builder;

use crate::generic::{create_parent, ensure_output_absent, validate_output};
use crate::{ExportError, ExportManifest, Exporter, ReceiptReference};

const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
struct UniqueJson(Value);
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = UniqueJson;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("unambiguous JSON")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueJson(Value::Bool(value)))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueJson(Value::Number(number)))
                    .ok_or_else(|| E::custom("invalid JSON number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueJson(Value::String(value.into())))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueJson(Value::String(value)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueJson>()? {
                    values.push(value.0);
                }
                Ok(UniqueJson(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("ambiguous JSON object"));
                    }
                    values.insert(key, map.next_value::<UniqueJson>()?.0);
                }
                Ok(UniqueJson(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(JsonVisitor)
    }
}

fn parse(bytes: &[u8], package: &LoadedSkillPackage) -> Result<ReceiptReference, ExportError> {
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err(ExportError::InvalidReceipt);
    }
    let value = serde_json::from_slice::<UniqueJson>(bytes)
        .map_err(|_| ExportError::InvalidReceipt)?
        .0;
    validate_json(SchemaId::ReceiptV1, &value).map_err(|_| ExportError::InvalidReceipt)?;
    if value["status"] != "succeeded"
        || value["assertions"]
            .as_array()
            .expect("validated assertions")
            .iter()
            .any(|item| item["passed"] != true)
        || value["policy_decisions"]
            .as_array()
            .expect("validated policy")
            .iter()
            .any(|item| item["allowed"] != true)
    {
        return Err(ExportError::InvalidReceipt);
    }
    let workflow =
        serde_json::to_value(&package.workflow).map_err(|_| ExportError::InvalidReceipt)?;
    let declared = workflow["steps"].as_array().expect("workflow steps");
    let steps = value["steps"].as_array().expect("validated steps");
    if steps.len() != declared.len()
        || steps.iter().zip(declared).any(|(step, expected)| {
            step["step_id"] != expected["id"]
                || step["status"] != "succeeded"
                || (!step["exit_code"].is_null() && step["exit_code"] != 0)
                || (matches!(expected["action"].as_str(), Some("exec" | "script"))
                    && step["exit_code"] != 0)
        })
    {
        return Err(ExportError::InvalidReceipt);
    }
    Ok(ReceiptReference {
        run_id: value["run_id"].as_str().expect("validated run id").into(),
        skill_hash: value["skill_hash"]
            .as_str()
            .expect("validated skill hash")
            .into(),
        receipt_sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        provenance: "not-authenticated",
    })
}

/// Match successful Receipt metadata to the actual exported package bytes.
/// This does not authenticate the caller, signer, provider or execution.
pub fn export_with_receipt(
    exporter: &dyn Exporter,
    package: &LoadedSkillPackage,
    output: &Path,
    bytes: &[u8],
) -> Result<ExportManifest, ExportError> {
    let reference = parse(bytes, package)?;
    validate_output(package, output)?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_parent(parent, output)?;
    ensure_output_absent(output)?;
    let stage = Builder::new()
        .prefix(".skilltape-receipt-export-")
        .tempdir_in(parent)
        .map_err(|source| ExportError::Io {
            path: parent.to_owned(),
            source,
        })?;
    let internal = stage.path().join("export");
    let mut manifest = exporter.export(package, &internal)?;
    if manifest.package_hash != reference.skill_hash {
        return Err(ExportError::ReceiptMismatch);
    }
    ensure_output_absent(output)?;
    publish_directory_noreplace(&internal, output).map_err(|source| ExportError::Io {
        path: output.to_owned(),
        source,
    })?;
    manifest.receipt = Some(reference);
    Ok(manifest)
}
