use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use skilltape_core::{DiagnosticLevel, LoadedSkillPackage, SkillPackage};
use skilltape_tape::{TapeEvent, TapeStore, TapeStoreError};
use thiserror::Error;

pub const CONSOLE_SCHEMA_V1: &str = "skilltape.dev/console/v1";
pub const WORKSPACE_ID: &str = "default";

const STORAGE_DIR: &str = ".skilltape";
const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 100;
const MAX_EVENT_BYTES: u64 = 16 * 1024 * 1024;
const REQUIRED_PACKAGE_FILES: [&str; 6] = [
    "skilltape.yaml",
    "workflow.yaml",
    "permissions.json",
    "skilltape.lock",
    "SKILL.md",
    "README.md",
];

#[derive(Clone, Debug)]
pub struct ConsoleReadModel {
    root: PathBuf,
}

#[derive(Debug, Error)]
pub enum ReadModelError {
    #[error("workspace root is invalid")]
    InvalidRoot,
    #[error("requested identifier is unsafe")]
    UnsafeId,
    #[error("requested path is unsafe")]
    UnsafePath,
    #[error("resource was not found")]
    NotFound,
    #[error("stored document is invalid")]
    InvalidDocument,
    #[error("stored resource could not be read")]
    Io(#[from] io::Error),
}

#[derive(Clone, Debug, Serialize)]
pub struct Collection<T> {
    pub schema: &'static str,
    pub items: Vec<T>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceSummary {
    pub id: &'static str,
    pub name: String,
    pub tape_count: usize,
    pub skill_count: usize,
    pub run_count: usize,
    pub receipt_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TapeSummary {
    pub id: String,
    pub schema: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub platform: String,
    pub workspace_root: String,
    pub event_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TapeEvents {
    pub schema: &'static str,
    pub tape_id: String,
    pub events: Vec<TapeEvent>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileSummary {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticSummary {
    pub code: String,
    pub level: &'static str,
    pub file: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LintSummary {
    pub files_checked: usize,
    pub errors: Vec<DiagnosticSummary>,
    pub warnings: Vec<DiagnosticSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillDiff {
    pub schema: &'static str,
    pub id: String,
    pub package_path: String,
    pub manifest: Value,
    pub workflow: Value,
    pub permissions: Value,
    pub lockfile: Value,
    pub files: Vec<FileSummary>,
    pub lint: LintSummary,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoredDocument {
    pub schema: &'static str,
    pub id: String,
    pub document: Value,
}

#[derive(Clone, Debug)]
pub struct StoredRunEvent {
    pub sequence: u64,
    pub document: Value,
}

impl ConsoleReadModel {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ReadModelError> {
        let root = root.as_ref();
        let metadata = fs::symlink_metadata(root).map_err(|_| ReadModelError::InvalidRoot)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ReadModelError::InvalidRoot);
        }
        let root = root
            .canonicalize()
            .map_err(|_| ReadModelError::InvalidRoot)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn workspaces(&self) -> Result<Collection<WorkspaceSummary>, ReadModelError> {
        let summary = WorkspaceSummary {
            id: WORKSPACE_ID,
            name: self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("workspace")
                .to_owned(),
            tape_count: self.count_directories(&self.storage_child("tapes")?)?,
            skill_count: self.skill_ids()?.len(),
            run_count: self.count_directories(&self.storage_child("runs")?)?,
            receipt_count: self.count_files(&self.storage_child("receipts")?, Some(".json"))?,
        };
        Ok(Collection {
            schema: CONSOLE_SCHEMA_V1,
            items: vec![summary],
            offset: 0,
            limit: 1,
            total: 1,
            next_offset: None,
        })
    }

    pub fn tapes(
        &self,
        workspace_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Collection<TapeSummary>, ReadModelError> {
        self.ensure_workspace(workspace_id)?;
        let directory = self.storage_child("tapes")?;
        let mut entries = self.directory_names(&directory)?;
        entries.sort();
        let total = entries.len();
        let items = entries
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|id| self.tape_summary(&id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(collection(items, offset, limit, total))
    }

    pub fn tape_events(
        &self,
        tape_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<TapeEvents, ReadModelError> {
        validate_id(tape_id)?;
        let tape_path = self.storage_child("tapes")?.join(tape_id);
        self.ensure_safe_path(&tape_path)?;
        reject_symlink_tree(&tape_path)?;
        let store = TapeStore::open(&tape_path).map_err(map_tape_error)?;
        let events = store
            .read_events()
            .map_err(map_tape_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_tape_error)?;
        let total = events.len();
        let page: Vec<TapeEvent> = events.into_iter().skip(offset).take(limit).collect();
        let next = offset.saturating_add(page.len());
        let next_offset = (next < total).then_some(next);
        Ok(TapeEvents {
            schema: CONSOLE_SCHEMA_V1,
            tape_id: tape_id.to_owned(),
            events: page,
            offset,
            limit,
            total,
            next_offset,
        })
    }

    pub fn skill_diff(&self, skill_id: &str) -> Result<SkillDiff, ReadModelError> {
        validate_id(skill_id)?;
        let package_path = self.find_skill(skill_id)?;
        reject_symlink_tree(&package_path)?;
        let package =
            SkillPackage::load(&package_path).map_err(|_| ReadModelError::InvalidDocument)?;
        let lint = package.lint(false);
        let files = REQUIRED_PACKAGE_FILES
            .iter()
            .map(|relative| self.file_summary(&package, relative))
            .collect::<Result<Vec<_>, _>>()?;
        let relative_package = package
            .root
            .strip_prefix(&self.root)
            .map_err(|_| ReadModelError::UnsafePath)?
            .to_string_lossy()
            .replace('\\', "/");
        Ok(SkillDiff {
            schema: CONSOLE_SCHEMA_V1,
            id: skill_id.to_owned(),
            package_path: relative_package,
            manifest: serde_json::to_value(&package.manifest)
                .map_err(|_| ReadModelError::InvalidDocument)?,
            workflow: serde_json::to_value(&package.workflow)
                .map_err(|_| ReadModelError::InvalidDocument)?,
            permissions: serde_json::to_value(&package.permissions)
                .map_err(|_| ReadModelError::InvalidDocument)?,
            lockfile: serde_json::to_value(&package.lockfile)
                .map_err(|_| ReadModelError::InvalidDocument)?,
            files,
            lint: lint_summary(lint),
        })
    }

    pub fn run(&self, run_id: &str) -> Result<StoredDocument, ReadModelError> {
        validate_id(run_id)?;
        let path = self.storage_child("runs")?.join(run_id).join("run.json");
        self.read_document(&path, run_id, "run")
    }

    pub fn receipt(&self, receipt_id: &str) -> Result<StoredDocument, ReadModelError> {
        validate_id(receipt_id)?;
        let path = self
            .storage_child("receipts")?
            .join(format!("{receipt_id}.json"));
        self.read_document(&path, receipt_id, "receipt")
    }

    pub fn run_events(
        &self,
        run_id: &str,
        last_event_id: Option<u64>,
    ) -> Result<Vec<StoredRunEvent>, ReadModelError> {
        validate_id(run_id)?;
        let path = self
            .storage_child("runs")?
            .join(run_id)
            .join("events.jsonl");
        self.ensure_safe_path(&path)?;
        let metadata = fs::metadata(&path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReadModelError::NotFound,
            _ => ReadModelError::Io(error),
        })?;
        let link_metadata = fs::symlink_metadata(&path).map_err(ReadModelError::Io)?;
        if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
            return Err(ReadModelError::UnsafePath);
        }
        if metadata.len() > MAX_EVENT_BYTES {
            return Err(ReadModelError::InvalidDocument);
        }
        let file = fs::File::open(&path).map_err(ReadModelError::Io)?;
        let mut events = Vec::new();
        let mut previous = None;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(ReadModelError::Io)?;
            if line.trim().is_empty() {
                continue;
            }
            let document: Value =
                serde_json::from_str(&line).map_err(|_| ReadModelError::InvalidDocument)?;
            let sequence = document
                .get("sequence")
                .and_then(Value::as_u64)
                .ok_or(ReadModelError::InvalidDocument)?;
            if previous.is_some_and(|previous| sequence <= previous) {
                return Err(ReadModelError::InvalidDocument);
            }
            previous = Some(sequence);
            if last_event_id.is_none_or(|last| sequence > last) {
                events.push(StoredRunEvent { sequence, document });
            }
        }
        Ok(events)
    }

    pub fn last_run_sequence(&self, run_id: &str) -> Result<Option<u64>, ReadModelError> {
        validate_id(run_id)?;
        let path = self
            .storage_child("runs")?
            .join(run_id)
            .join("events.jsonl");
        self.ensure_safe_path(&path)?;
        let link_metadata = fs::symlink_metadata(&path).map_err(ReadModelError::Io)?;
        if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
            return Err(ReadModelError::UnsafePath);
        }
        let file = fs::File::open(&path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReadModelError::NotFound,
            _ => ReadModelError::Io(error),
        })?;
        let mut last = None;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(ReadModelError::Io)?;
            if line.trim().is_empty() {
                continue;
            }
            let document: Value =
                serde_json::from_str(&line).map_err(|_| ReadModelError::InvalidDocument)?;
            last = Some(
                document
                    .get("sequence")
                    .and_then(Value::as_u64)
                    .ok_or(ReadModelError::InvalidDocument)?,
            );
        }
        Ok(last)
    }

    fn ensure_workspace(&self, workspace_id: &str) -> Result<(), ReadModelError> {
        (workspace_id == WORKSPACE_ID)
            .then_some(())
            .ok_or(ReadModelError::NotFound)
    }

    fn storage_child(&self, name: &str) -> Result<PathBuf, ReadModelError> {
        validate_id(name)?;
        let path = self.root.join(STORAGE_DIR).join(name);
        self.ensure_safe_path(&path)?;
        Ok(path)
    }

    fn ensure_safe_path(&self, path: &Path) -> Result<(), ReadModelError> {
        if !path.starts_with(&self.root) {
            return Err(ReadModelError::UnsafePath);
        }
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| ReadModelError::UnsafePath)?;
        let mut current = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(ReadModelError::UnsafePath);
            };
            current.push(name);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(ReadModelError::UnsafePath)
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(error) => return Err(ReadModelError::Io(error)),
            }
        }
        Ok(())
    }

    fn directory_names(&self, path: &Path) -> Result<Vec<String>, ReadModelError> {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(ReadModelError::Io(error)),
        };
        let mut names = BTreeSet::new();
        for entry in entries {
            let entry = entry.map_err(ReadModelError::Io)?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(ReadModelError::Io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if metadata.is_dir() && !metadata.file_type().is_symlink() && validate_id(&name).is_ok()
            {
                names.insert(name);
            }
        }
        Ok(names.into_iter().collect())
    }

    fn count_directories(&self, path: &Path) -> Result<usize, ReadModelError> {
        Ok(self.directory_names(path)?.len())
    }

    fn count_files(&self, path: &Path, suffix: Option<&str>) -> Result<usize, ReadModelError> {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(ReadModelError::Io(error)),
        };
        let mut count = 0;
        for entry in entries {
            let entry = entry.map_err(ReadModelError::Io)?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(ReadModelError::Io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && suffix.is_none_or(|suffix| name.ends_with(suffix))
            {
                count += 1;
            }
        }
        Ok(count)
    }

    fn skill_ids(&self) -> Result<Vec<String>, ReadModelError> {
        let mut ids = BTreeSet::new();
        for root in [
            self.root.join("skills"),
            self.root.join(STORAGE_DIR).join("skills"),
        ] {
            self.ensure_safe_path(&root)?;
            ids.extend(self.directory_names(&root)?);
        }
        Ok(ids.into_iter().collect())
    }

    fn tape_summary(&self, id: &str) -> Result<TapeSummary, ReadModelError> {
        let path = self.storage_child("tapes")?.join(id);
        self.ensure_safe_path(&path)?;
        reject_symlink_tree(&path)?;
        let store = TapeStore::open(path).map_err(map_tape_error)?;
        let manifest = store.read_manifest().map_err(map_tape_error)?;
        Ok(TapeSummary {
            id: manifest.id.clone(),
            schema: manifest.schema,
            started_at_ms: manifest.started_at_ms,
            finished_at_ms: manifest.finished_at_ms,
            platform: manifest.platform,
            workspace_root: manifest.workspace_root,
            event_count: manifest.event_count,
        })
    }

    fn find_skill(&self, id: &str) -> Result<PathBuf, ReadModelError> {
        let candidates = [
            self.root.join("skills").join(id),
            self.root.join(STORAGE_DIR).join("skills").join(id),
            self.root.join(id),
        ];
        for path in candidates {
            self.ensure_safe_path(&path)?;
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(ReadModelError::UnsafePath)
                }
                Ok(metadata) if metadata.is_dir() => return Ok(path),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(ReadModelError::Io(error)),
            }
        }
        Err(ReadModelError::NotFound)
    }

    fn file_summary(
        &self,
        package: &LoadedSkillPackage,
        relative: &str,
    ) -> Result<FileSummary, ReadModelError> {
        let path = package.root.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|_| ReadModelError::InvalidDocument)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ReadModelError::InvalidDocument);
        }
        let contents = fs::read(&path).map_err(|_| ReadModelError::InvalidDocument)?;
        Ok(FileSummary {
            path: relative.to_owned(),
            bytes: contents.len(),
            sha256: sha256(&contents),
        })
    }

    fn read_document(
        &self,
        path: &Path,
        id: &str,
        kind: &str,
    ) -> Result<StoredDocument, ReadModelError> {
        self.ensure_safe_path(path)?;
        let metadata = fs::symlink_metadata(path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReadModelError::NotFound,
            _ => ReadModelError::Io(error),
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ReadModelError::UnsafePath);
        }
        let bytes = fs::read(path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReadModelError::NotFound,
            _ => ReadModelError::Io(error),
        })?;
        let document =
            serde_json::from_slice(&bytes).map_err(|_| ReadModelError::InvalidDocument)?;
        let schema = match kind {
            "run" => "skilltape.dev/run/v1",
            "receipt" => "skilltape.dev/receipt/v1",
            _ => CONSOLE_SCHEMA_V1,
        };
        Ok(StoredDocument {
            schema,
            id: id.to_owned(),
            document,
        })
    }
}

fn collection<T>(items: Vec<T>, offset: usize, limit: usize, total: usize) -> Collection<T> {
    let next = offset.saturating_add(items.len());
    let next_offset = (next < total).then_some(next);
    Collection {
        schema: CONSOLE_SCHEMA_V1,
        items,
        offset,
        limit,
        total,
        next_offset,
    }
}

pub fn normalize_page(
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<(usize, usize), ReadModelError> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(ReadModelError::InvalidDocument);
    }
    Ok((offset, limit))
}

pub fn validate_id(id: &str) -> Result<(), ReadModelError> {
    if id.is_empty()
        || id == "."
        || id == ".."
        || id.contains(['/', '\\', ':'])
        || id.contains('\0')
        || !matches!(
            Path::new(id).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
    {
        return Err(ReadModelError::UnsafeId);
    }
    Ok(())
}

fn reject_symlink_tree(root: &Path) -> Result<(), ReadModelError> {
    let mut entries = vec![root.to_owned()];
    while let Some(path) = entries.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReadModelError::NotFound,
            _ => ReadModelError::Io(error),
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ReadModelError::UnsafePath);
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(ReadModelError::Io)? {
                entries.push(entry.map_err(ReadModelError::Io)?.path());
            }
        }
    }
    Ok(())
}

fn lint_summary(report: skilltape_core::LintReport) -> LintSummary {
    let convert = |diagnostic: skilltape_core::Diagnostic| DiagnosticSummary {
        code: diagnostic.code,
        level: match diagnostic.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
        },
        file: diagnostic.file,
        path: diagnostic.path,
        message: diagnostic.message,
    };
    LintSummary {
        files_checked: report.files_checked,
        errors: report.errors.into_iter().map(convert).collect(),
        warnings: report.warnings.into_iter().map(convert).collect(),
    }
}

fn sha256(contents: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(contents);
    format!("{:x}", hasher.finalize())
}

fn map_tape_error(error: TapeStoreError) -> ReadModelError {
    match error {
        TapeStoreError::Io(error) => {
            if error.kind() == io::ErrorKind::NotFound {
                ReadModelError::NotFound
            } else {
                ReadModelError::Io(error)
            }
        }
        TapeStoreError::InvalidRoot { .. } | TapeStoreError::AlreadyExists { .. } => {
            ReadModelError::NotFound
        }
        _ => ReadModelError::InvalidDocument,
    }
}
