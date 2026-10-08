use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use skilltape_core::LoadedSkillPackage;
use skilltape_schema::Step;
use tempfile::{Builder, TempDir};
use thiserror::Error;

// Metadata preflight only; copy-time quotas require a separate bounded-copy slice.
const INPUT_MAX_ENTRIES: u64 = 10_000;
const INPUT_MAX_DEPTH: u64 = 64;
const INPUT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const INPUT_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub(crate) enum WorkspaceError {
    #[error("input root is not a regular directory: {path}")]
    InvalidInputRoot { path: PathBuf },
    #[error("input capacity exceeded: {dimension} limit is {limit}")]
    InputCapacity { dimension: &'static str, limit: u64 },
    #[error("workspace path is unsafe: {path}")]
    UnsafePath { path: String },
    #[error("symlinks are not allowed in a replay workspace: {path}")]
    Symlink { path: PathBuf },
    #[error("referenced package script is missing: {path}")]
    MissingScript { path: String },
    #[error("declared output is missing from the replay workspace: {path}")]
    MissingOutput { path: String },
    #[error("output root already exists: {path}")]
    OutputExists { path: PathBuf },
    #[error("cannot copy directory into itself or a descendant: {source_path} -> {destination}")]
    CopyIntoSource {
        source_path: PathBuf,
        destination: PathBuf,
    },
    #[error("workspace I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub(crate) struct ReplayWorkspace {
    _tempdir: TempDir,
    root: PathBuf,
}

impl ReplayWorkspace {
    pub(crate) fn prepare(
        package: &LoadedSkillPackage,
        input_root: &Path,
    ) -> Result<Self, WorkspaceError> {
        ensure_input_root(input_root)?;

        let tempdir = tempfile::tempdir().map_err(|source| WorkspaceError::Io {
            path: input_root.to_path_buf(),
            source,
        })?;
        let root = tempdir.path().to_path_buf();

        copy_entry(input_root, &root.join("inputs"))?;
        copy_referenced_scripts(package, &root)?;

        Ok(Self {
            _tempdir: tempdir,
            root,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn resolve(&self, relative: &str) -> Result<PathBuf, WorkspaceError> {
        resolve_under(&self.root, relative)
    }

    pub(crate) fn ensure_safe_path(&self, path: &Path) -> Result<(), WorkspaceError> {
        ensure_no_symlink_ancestors(path)
    }

    pub(crate) fn materialize_outputs(
        &self,
        output_root: &Path,
        paths: &[String],
    ) -> Result<(), WorkspaceError> {
        if paths.is_empty() {
            return Ok(());
        }

        ensure_no_symlink_ancestors(output_root)?;
        if symlink_metadata(output_root)?.is_some() {
            return Err(WorkspaceError::OutputExists {
                path: output_root.to_path_buf(),
            });
        }

        let parent = output_root.parent().unwrap_or_else(|| Path::new("."));
        ensure_no_symlink_ancestors(parent)?;
        fs::create_dir_all(parent).map_err(|source| WorkspaceError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        ensure_no_symlink_ancestors(parent)?;

        let staging = Builder::new()
            .prefix(".skilltape-output-")
            .tempdir_in(parent)
            .map_err(|source| WorkspaceError::Io {
                path: parent.to_path_buf(),
                source,
            })?;

        let mut destinations = BTreeSet::new();
        for relative in paths {
            let source = self.resolve(relative)?;
            let source_metadata =
                symlink_metadata(&source)?.ok_or_else(|| WorkspaceError::MissingOutput {
                    path: relative.clone(),
                })?;
            if source_metadata.is_symlink() {
                return Err(WorkspaceError::Symlink { path: source });
            }
            let destination_relative = materialized_relative(relative)?;
            if !destinations.insert(destination_relative.clone()) {
                continue;
            }
            let destination = staging.path().join(destination_relative);
            copy_entry(&source, &destination)?;
        }

        if symlink_metadata(output_root)?.is_some() {
            return Err(WorkspaceError::OutputExists {
                path: output_root.to_path_buf(),
            });
        }
        crate::publish_directory_noreplace(staging.path(), output_root).map_err(|source| {
            WorkspaceError::Io {
                path: output_root.to_path_buf(),
                source,
            }
        })?;

        Ok(())
    }
}

fn materialized_relative(relative: &str) -> Result<String, WorkspaceError> {
    let relative = relative
        .strip_prefix("outputs/")
        .unwrap_or(relative)
        .to_owned();
    validate_relative_path(&relative)?;
    Ok(relative)
}

pub(crate) fn resolve_under(root: &Path, relative: &str) -> Result<PathBuf, WorkspaceError> {
    validate_relative_path(relative)?;
    let path = root.join(relative);
    if !path.starts_with(root) {
        return Err(WorkspaceError::UnsafePath {
            path: relative.to_owned(),
        });
    }
    Ok(path)
}

pub(crate) fn copy_path(from: &Path, to: &Path) -> Result<(), WorkspaceError> {
    copy_entry(from, to)
}

pub(crate) fn move_path(from: &Path, to: &Path) -> Result<(), WorkspaceError> {
    let metadata = symlink_metadata(from)?.ok_or_else(|| WorkspaceError::Io {
        path: from.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "path does not exist"),
    })?;
    if metadata.is_symlink() {
        return Err(WorkspaceError::Symlink {
            path: from.to_path_buf(),
        });
    }
    ensure_no_symlink_ancestors(from)?;
    ensure_no_symlink_ancestors(to)?;
    if symlink_metadata(to)?.is_some() {
        return Err(WorkspaceError::OutputExists {
            path: to.to_path_buf(),
        });
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|source| WorkspaceError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::rename(from, to).map_err(|source| WorkspaceError::Io {
        path: to.to_path_buf(),
        source,
    })
}

pub(crate) fn make_directory(path: &Path) -> Result<(), WorkspaceError> {
    ensure_no_symlink_ancestors(path)?;
    if let Some(metadata) = symlink_metadata(path)? {
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(WorkspaceError::OutputExists {
                path: path.to_path_buf(),
            });
        }
        return Ok(());
    }
    fs::create_dir_all(path).map_err(|source| WorkspaceError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), WorkspaceError> {
    let bytes = path.as_bytes();
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || (bytes.len() >= 2 && bytes[1] == b':')
        || path
            .split(['/', '\\'])
            .any(|component| component == ".." || component.is_empty())
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(WorkspaceError::UnsafePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn ensure_input_root(input_root: &Path) -> Result<(), WorkspaceError> {
    let metadata =
        symlink_metadata(input_root)?.ok_or_else(|| WorkspaceError::InvalidInputRoot {
            path: input_root.to_path_buf(),
        })?;
    if metadata.is_symlink() || !metadata.is_dir() {
        return Err(WorkspaceError::InvalidInputRoot {
            path: input_root.to_path_buf(),
        });
    }
    ensure_no_symlink_ancestors(input_root)
}

/// Count metadata without reading input bytes or collecting an unbounded directory list.
pub(crate) fn validate_input_capacity(input_root: &Path) -> Result<(), WorkspaceError> {
    let root = canonical_input_root(input_root)?;
    let mut entries = 0;
    let mut total_bytes = 0;
    inspect_input_directory(&root, &root, 0, &mut entries, &mut total_bytes)
}

fn canonical_input_root(input_root: &Path) -> Result<PathBuf, WorkspaceError> {
    let expected = lexical_workspace_path(input_root)?;
    let resolved = expected
        .canonicalize()
        .map_err(|source| WorkspaceError::Io {
            path: input_root.to_path_buf(),
            source,
        })?;
    if resolved != expected || !resolved.starts_with(&expected) {
        return Err(WorkspaceError::InvalidInputRoot {
            path: input_root.to_path_buf(),
        });
    }
    ensure_input_root(&resolved)?;
    Ok(resolved)
}

fn lexical_workspace_path(input_root: &Path) -> Result<PathBuf, WorkspaceError> {
    if input_root
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(WorkspaceError::UnsafePath {
            path: input_root.to_string_lossy().into_owned(),
        });
    }
    let absolute = std::path::absolute(input_root).map_err(|source| WorkspaceError::Io {
        path: input_root.to_path_buf(),
        source,
    })?;
    let mut expected = PathBuf::new();
    for part in absolute.components() {
        expected.push(part);
        // Preserve only the already-approved macOS /tmp, /var and /etc aliases.
        if is_allowed_system_alias(&expected) {
            expected = expected
                .canonicalize()
                .map_err(|source| WorkspaceError::Io {
                    path: input_root.to_path_buf(),
                    source,
                })?;
        }
    }
    Ok(expected)
}

fn inspect_input_directory(
    root: &Path,
    current: &Path,
    depth: u64,
    entries: &mut u64,
    total_bytes: &mut u64,
) -> Result<(), WorkspaceError> {
    let resolved = current
        .canonicalize()
        .map_err(|source| WorkspaceError::Io {
            path: current.to_path_buf(),
            source,
        })?;
    if resolved != current || !resolved.starts_with(root) {
        return Err(WorkspaceError::UnsafePath {
            path: current.to_string_lossy().into_owned(),
        });
    }
    ensure_no_symlink_ancestors(current)?;
    let directory = fs::read_dir(&resolved).map_err(|source| WorkspaceError::Io {
        path: current.to_path_buf(),
        source,
    })?;
    for entry in directory {
        let path = entry
            .map_err(|source| WorkspaceError::Io {
                path: current.to_path_buf(),
                source,
            })?
            .path();
        *entries += 1;
        check_input_limit(*entries, INPUT_MAX_ENTRIES, "entries")?;
        check_input_limit(depth + 1, INPUT_MAX_DEPTH, "depth")?;
        ensure_no_symlink_ancestors(&path)?;
        let metadata = symlink_metadata(&path)?.ok_or_else(|| WorkspaceError::Io {
            path: path.clone(),
            source: io::Error::new(io::ErrorKind::NotFound, "input entry disappeared"),
        })?;
        if metadata.is_symlink() {
            return Err(WorkspaceError::Symlink { path });
        }
        if metadata.is_dir() {
            inspect_input_directory(root, &path, depth + 1, entries, total_bytes)?;
        } else if metadata.is_file() {
            check_input_limit(metadata.len(), INPUT_MAX_FILE_BYTES, "file_bytes")?;
            // Both addends have already been capped; the sum cannot overflow u64.
            *total_bytes += metadata.len();
            check_input_limit(*total_bytes, INPUT_MAX_TOTAL_BYTES, "total_bytes")?;
        } else {
            return Err(WorkspaceError::Io {
                path,
                source: io::Error::new(io::ErrorKind::InvalidInput, "unsupported input entry"),
            });
        }
    }
    Ok(())
}

fn check_input_limit(
    value: u64,
    limit: u64,
    dimension: &'static str,
) -> Result<(), WorkspaceError> {
    if value > limit {
        return Err(WorkspaceError::InputCapacity { dimension, limit });
    }
    Ok(())
}

fn copy_referenced_scripts(
    package: &LoadedSkillPackage,
    workspace_root: &Path,
) -> Result<(), WorkspaceError> {
    let mut paths = BTreeSet::new();
    for step in &package.workflow.steps {
        if let Step::Script(step) = step {
            paths.insert(step.path.clone());
        }
    }
    for script in &package.lockfile.scripts {
        if let Some(path) = script.get("path").and_then(serde_json::Value::as_str) {
            paths.insert(path.to_owned());
        }
    }

    for relative in paths {
        validate_relative_path(&relative)?;
        let source = package.root.join(&relative);
        ensure_no_symlink_ancestors(&source)?;
        if symlink_metadata(&source)?.is_none() {
            return Err(WorkspaceError::MissingScript { path: relative });
        }
        copy_entry(&source, &workspace_root.join(&relative))?;
    }
    Ok(())
}

fn copy_entry(source: &Path, destination: &Path) -> Result<(), WorkspaceError> {
    let metadata = symlink_metadata(source)?.ok_or_else(|| WorkspaceError::Io {
        path: source.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "path does not exist"),
    })?;
    if metadata.is_symlink() {
        return Err(WorkspaceError::Symlink {
            path: source.to_path_buf(),
        });
    }
    ensure_no_symlink_ancestors(source)?;
    ensure_no_symlink_ancestors(destination)?;

    if metadata.is_dir() {
        if destination == source || destination.starts_with(source) {
            return Err(WorkspaceError::CopyIntoSource {
                source_path: source.to_path_buf(),
                destination: destination.to_path_buf(),
            });
        }
        if let Some(destination_metadata) = symlink_metadata(destination)? {
            if destination_metadata.is_symlink() {
                return Err(WorkspaceError::Symlink {
                    path: destination.to_path_buf(),
                });
            }
            if !destination_metadata.is_dir() {
                return Err(WorkspaceError::OutputExists {
                    path: destination.to_path_buf(),
                });
            }
        }
        fs::create_dir_all(destination).map_err(|source| WorkspaceError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        let source_path = source.to_path_buf();
        let directory = fs::read_dir(source).map_err(|error| WorkspaceError::Io {
            path: source_path.clone(),
            source: error,
        })?;
        let mut entries = directory
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|error| WorkspaceError::Io {
                        path: source.to_path_buf(),
                        source: error,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort();
        for entry in entries {
            let name = entry.file_name().ok_or_else(|| WorkspaceError::Io {
                path: entry.clone(),
                source: io::Error::new(io::ErrorKind::InvalidData, "entry has no file name"),
            })?;
            copy_entry(&entry, &destination.join(name))?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| WorkspaceError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        if symlink_metadata(destination)?.is_some() {
            return Err(WorkspaceError::OutputExists {
                path: destination.to_path_buf(),
            });
        }
        fs::copy(source, destination).map_err(|source| WorkspaceError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        fs::set_permissions(destination, metadata.permissions()).map_err(|source| {
            WorkspaceError::Io {
                path: destination.to_path_buf(),
                source,
            }
        })?;
    } else {
        return Err(WorkspaceError::Io {
            path: source.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "unsupported filesystem entry"),
        });
    }
    Ok(())
}

pub(crate) fn ensure_no_symlink_ancestors(path: &Path) -> Result<(), WorkspaceError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let Some(metadata) = symlink_metadata(&current)? else {
            break;
        };
        if metadata.is_symlink() && !is_allowed_system_alias(&current) {
            return Err(WorkspaceError::Symlink { path: current });
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn is_allowed_system_alias(path: &Path) -> bool {
    let Some(name) = path.to_str() else {
        return false;
    };
    if !matches!(name, "/etc" | "/tmp" | "/var") {
        return false;
    }
    path.canonicalize()
        .map(|canonical| {
            matches!(
                canonical.to_str(),
                Some("/private/etc" | "/private/tmp" | "/private/var")
            )
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn is_allowed_system_alias(_path: &Path) -> bool {
    false
}

fn symlink_metadata(path: &Path) -> Result<Option<fs::Metadata>, WorkspaceError> {
    let expected = lexical_workspace_path(path)?;
    let resolved = match expected.canonicalize() {
        Ok(resolved) => resolved,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            // A dangling link is an existing unsafe entry, not an absent target.
            return match fs::read_link(&expected) {
                Ok(_) => Err(WorkspaceError::Symlink {
                    path: path.to_path_buf(),
                }),
                Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(source) => Err(WorkspaceError::Io {
                    path: path.to_path_buf(),
                    source,
                }),
            };
        }
        Err(source) => {
            return Err(WorkspaceError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    if resolved != expected || !resolved.starts_with(&expected) {
        return Err(WorkspaceError::Symlink {
            path: path.to_path_buf(),
        });
    }
    match fs::symlink_metadata(&resolved) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(WorkspaceError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}
