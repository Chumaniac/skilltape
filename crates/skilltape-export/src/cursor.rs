use std::fs;
use std::path::Path;

use tempfile::Builder;

use crate::generic::{create_parent, ensure_output_absent, validate_output, GenericExporter};
use crate::{ExportError, ExportManifest, Exporter};
use skilltape_core::LoadedSkillPackage;

const TARGET_ID: &str = "cursor";

#[derive(Clone, Copy, Debug, Default)]
pub struct CursorExporter;

impl Exporter for CursorExporter {
    fn target_id(&self) -> &'static str {
        TARGET_ID
    }

    fn export(
        &self,
        package: &LoadedSkillPackage,
        output: &Path,
    ) -> Result<ExportManifest, ExportError> {
        validate_name(&package.manifest.name)?;
        validate_output(package, output)?;

        let output = output.to_owned();
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        create_parent(parent, &output)?;
        ensure_output_absent(&output)?;

        let staging = Builder::new()
            .prefix(".skilltape-cursor-")
            .tempdir_in(parent)
            .map_err(|source| ExportError::Io {
                path: parent.to_owned(),
                source,
            })?;
        let package_output = staging
            .path()
            .join(".cursor")
            .join("skills")
            .join(&package.manifest.name);
        let generic_manifest = GenericExporter.export(package, &package_output)?;

        ensure_output_absent(&output)?;
        fs::rename(staging.path(), &output).map_err(|source| ExportError::Io {
            path: output.clone(),
            source,
        })?;

        let prefix = format!(".cursor/skills/{}/", package.manifest.name);
        Ok(ExportManifest {
            target: TARGET_ID.to_owned(),
            files: generic_manifest
                .files
                .into_iter()
                .map(|file| format!("{prefix}{file}"))
                .collect(),
            package_hash: generic_manifest.package_hash,
        })
    }
}

fn validate_name(name: &str) -> Result<(), ExportError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
        })
    {
        return Err(ExportError::InvalidTargetName {
            name: name.to_owned(),
        });
    }
    Ok(())
}
