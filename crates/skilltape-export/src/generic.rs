use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use skilltape_core::LoadedSkillPackage;
use skilltape_core::SkillPackage;
use skilltape_runner::publish_directory_noreplace;
use skilltape_schema::Step;
use tempfile::Builder;

use crate::snapshot::{Snapshot, MAX_DEPTH, MAX_ENTRIES};
use crate::{ExportError, ExportManifest, Exporter};

const TARGET_ID: &str = "generic-agent-skill";
const REQUIRED_FILES: [&str; 6] = [
    "skilltape.yaml",
    "workflow.yaml",
    "permissions.json",
    "skilltape.lock",
    "SKILL.md",
    "README.md",
];
const OPTIONAL_FILES: [&str; 7] = [
    "compile.json",
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "NOTICE",
    "NOTICE.md",
    "COPYING",
];
const OPTIONAL_DIRECTORIES: [&str; 2] = ["fixtures", "receipts"];

#[derive(Clone, Copy, Debug, Default)]
pub struct GenericExporter;

impl Exporter for GenericExporter {
    fn target_id(&self) -> &'static str {
        TARGET_ID
    }

    fn export(
        &self,
        package: &LoadedSkillPackage,
        output: &Path,
    ) -> Result<ExportManifest, ExportError> {
        let lint = package.lint(false);
        if !lint.errors.is_empty() {
            return Err(ExportError::Lint {
                errors: lint.errors.len(),
            });
        }

        let files = export_files(package)?;
        let output = output.to_owned();
        validate_output(package, &output)?;
        let snapshot = Snapshot::capture(&package.root, &files)?;
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        create_parent(parent, &output)?;
        ensure_output_absent(&output)?;
        let staging = Builder::new()
            .prefix(".skilltape-export-")
            .tempdir_in(parent)
            .map_err(|source| ExportError::Io {
                path: parent.to_owned(),
                source,
            })?;
        snapshot.copy_to(staging.path())?;
        let staged = SkillPackage::load(staging.path()).map_err(|_| ExportError::ChangedSource)?;
        if !staged.lint(false).errors.is_empty() {
            return Err(ExportError::ChangedSource);
        }
        let package_hash = hash_files(staging.path(), &files)?;
        snapshot.check()?;
        if export_files(package)? != files {
            return Err(ExportError::ChangedSource);
        }

        ensure_output_absent(&output)?;
        publish_directory_noreplace(staging.path(), &output).map_err(|source| ExportError::Io {
            path: output.clone(),
            source,
        })?;

        Ok(ExportManifest {
            target: TARGET_ID.to_owned(),
            files,
            package_hash,
            receipt: None,
        })
    }
}

fn export_files(package: &LoadedSkillPackage) -> Result<Vec<String>, ExportError> {
    let mut files = BTreeSet::new();
    for relative in REQUIRED_FILES {
        add_file(package, relative, &mut files)?;
    }
    for relative in OPTIONAL_FILES {
        add_optional_file(package, relative, &mut files)?;
    }
    let mut entries = files.len();
    for relative in OPTIONAL_DIRECTORIES {
        collect_tree(package, relative, &mut files, &mut entries, false)?;
    }

    let mut referenced_scripts = BTreeSet::new();
    for step in &package.workflow.steps {
        if let Step::Script(step) = step {
            referenced_scripts.insert(step.path.clone());
        }
    }
    for script in &package.lockfile.scripts {
        if let Some(path) = script.get("path").and_then(serde_json::Value::as_str) {
            referenced_scripts.insert(path.to_owned());
        }
    }
    for relative in referenced_scripts {
        if !files.contains(&relative) {
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(ExportError::Capacity);
            }
        }
        add_file(package, &relative, &mut files)?;
    }

    if files.len() > MAX_ENTRIES {
        return Err(ExportError::Capacity);
    }
    Ok(files.into_iter().collect())
}

fn add_optional_file(
    package: &LoadedSkillPackage,
    relative: &str,
    files: &mut BTreeSet<String>,
) -> Result<(), ExportError> {
    validate_relative(relative)?;
    let path = package.root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(ExportError::SymlinkSource { path })
        }
        Ok(metadata) if metadata.is_file() => {
            files.insert(relative.to_owned());
            Ok(())
        }
        Ok(_) => Err(ExportError::UnsupportedSource { path }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ExportError::Io { path, source }),
    }
}

fn add_file(
    package: &LoadedSkillPackage,
    relative: &str,
    files: &mut BTreeSet<String>,
) -> Result<(), ExportError> {
    validate_relative(relative)?;
    let path = package.root.join(relative);
    ensure_source_ancestors(&package.root, relative)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(ExportError::SymlinkSource { path })
        }
        Ok(metadata) if metadata.is_file() => {
            files.insert(relative.to_owned());
            Ok(())
        }
        Ok(_) => Err(ExportError::UnsupportedSource { path }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(ExportError::MissingSource {
                path: relative.to_owned(),
            })
        }
        Err(source) => Err(ExportError::Io { path, source }),
    }
}

fn collect_tree(
    package: &LoadedSkillPackage,
    relative_root: &str,
    files: &mut BTreeSet<String>,
    observed: &mut usize,
    root_counted: bool,
) -> Result<(), ExportError> {
    validate_relative(relative_root)?;
    if relative_root.split('/').count() > MAX_DEPTH {
        return Err(ExportError::Capacity);
    }
    let root = package.root.join(relative_root);
    let metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(ExportError::Io { path: root, source }),
    };
    if metadata.file_type().is_symlink() {
        return Err(ExportError::SymlinkSource { path: root });
    }
    if !metadata.is_dir() {
        return Err(ExportError::UnsupportedSource { path: root });
    }
    if !root_counted {
        *observed += 1;
        if *observed > MAX_ENTRIES {
            return Err(ExportError::Capacity);
        }
    }

    let reader = fs::read_dir(&root).map_err(|source| ExportError::Io {
        path: root.clone(),
        source,
    })?;
    let mut entries = Vec::new();
    for entry in reader {
        *observed += 1;
        if *observed > MAX_ENTRIES {
            return Err(ExportError::Capacity);
        }
        entries.push(entry.map_err(|source| ExportError::Io {
            path: root.clone(),
            source,
        })?);
    }
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let child = path
            .strip_prefix(&package.root)
            .map_err(|_| ExportError::UnsafeSource {
                path: path.to_string_lossy().into_owned(),
            })?
            .to_str()
            .ok_or_else(|| ExportError::UnsafeSource {
                path: "non-UTF-8 export path".into(),
            })?
            .replace('\\', "/");
        let metadata = fs::symlink_metadata(&path).map_err(|source| ExportError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ExportError::SymlinkSource { path });
        }
        if metadata.is_dir() {
            collect_tree(package, &child, files, observed, true)?;
        } else if metadata.is_file() {
            files.insert(child);
            if files.len() > MAX_ENTRIES {
                return Err(ExportError::Capacity);
            }
        } else {
            return Err(ExportError::UnsupportedSource { path });
        }
    }
    Ok(())
}

fn hash_files(root: &Path, files: &[String]) -> Result<String, ExportError> {
    let mut hasher = Sha256::new();
    for relative in files {
        let path = root.join(relative);
        ensure_source_ancestors(root, relative)?;
        let mut file = fs::File::open(&path).map_err(|source| ExportError::Io {
            path: path.clone(),
            source,
        })?;
        let bytes = file
            .metadata()
            .map_err(|source| ExportError::Io {
                path: path.clone(),
                source,
            })?
            .len();
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(bytes.to_be_bytes());
        crate::snapshot::read_observed(&mut file, bytes, |chunk| {
            hasher.update(chunk);
            Ok(())
        })
        .map_err(|source| ExportError::Io {
            path: path.clone(),
            source,
        })?;
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

pub(crate) fn validate_output(
    package: &LoadedSkillPackage,
    output: &Path,
) -> Result<(), ExportError> {
    if output.as_os_str().is_empty()
        || output
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ExportError::UnsafeOutput {
            path: output.to_owned(),
        });
    }
    let output_absolute = absolute_path(output)?;
    if output_absolute == package.root
        || output_absolute.starts_with(&package.root)
        || !ancestors_are_safe(&output_absolute)
    {
        return Err(ExportError::UnsafeOutput {
            path: output.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn create_parent(parent: &Path, output: &Path) -> Result<(), ExportError> {
    if !ancestors_are_safe(parent) {
        return Err(ExportError::UnsafeOutput {
            path: output.to_owned(),
        });
    }
    fs::create_dir_all(parent).map_err(|source| ExportError::Io {
        path: parent.to_owned(),
        source,
    })?;
    if !ancestors_are_safe(parent) {
        return Err(ExportError::UnsafeOutput {
            path: output.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn ensure_output_absent(output: &Path) -> Result<(), ExportError> {
    match fs::symlink_metadata(output) {
        Ok(_) => Err(ExportError::OutputExists {
            path: output.to_owned(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ExportError::Io {
            path: output.to_owned(),
            source,
        }),
    }
}

pub(crate) fn ensure_source_ancestors(root: &Path, relative: &str) -> Result<(), ExportError> {
    let mut current = root.to_owned();
    for component in Path::new(relative).components() {
        let Component::Normal(name) = component else {
            return Err(ExportError::UnsafeSource {
                path: relative.to_owned(),
            });
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current).map_err(|source| ExportError::Io {
            path: current.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ExportError::SymlinkSource { path: current });
        }
    }
    Ok(())
}

fn validate_relative(relative: &str) -> Result<(), ExportError> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative.starts_with('/')
        || relative.as_bytes().get(1) == Some(&b':')
        || Path::new(relative)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExportError::UnsafeSource {
            path: relative.to_owned(),
        });
    }
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf, ExportError> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|source| ExportError::Io {
                path: path.to_owned(),
                source,
            })
    }
}

fn ancestors_are_safe(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata)
                if metadata.file_type().is_symlink() && !is_allowed_system_alias(&current) =>
            {
                return false;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return false,
        }
    }
    true
}

fn is_allowed_system_alias(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if path != Path::new("/etc") && path != Path::new("/tmp") && path != Path::new("/var") {
            return false;
        }
        path.canonicalize().is_ok_and(|canonical| {
            matches!(
                canonical.to_str(),
                Some("/private/etc") | Some("/private/tmp") | Some("/private/var")
            )
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}
