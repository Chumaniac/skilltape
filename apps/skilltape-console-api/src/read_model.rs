use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
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
const MAX_TAPE_EVENTS: u64 = 100_000;
const MAX_TAPE_MANIFEST_BYTES: u64 = 64 * 1024;
// Match the existing Receipt-context budget; larger saved documents need a
// paginated format before Console can safely inspect them.
const MAX_STORED_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
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
    #[error("delivery inspection is unavailable on this platform")]
    UnsupportedPlatform,
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
    // Address the stored directory without rewriting the Tape's recorded identity.
    pub id: String,
    pub recorded_id: String,
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
    pub policy_plan: Option<skilltape_verify::PolicyPlan>,
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
        let store = self.checked_tape(tape_id)?;
        let limit = limit.clamp(1, MAX_PAGE_SIZE);
        let mut page = Vec::new();
        let mut total = 0;
        // Validate the complete bounded stream, retaining only the requested page.
        for event in store
            .read_events_bounded(MAX_EVENT_BYTES, MAX_TAPE_EVENTS)
            .map_err(map_tape_error)?
        {
            let event = event.map_err(map_tape_error)?;
            if total >= offset && page.len() < limit {
                page.push(event);
            }
            total += 1;
        }
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

    fn checked_tape(&self, tape_id: &str) -> Result<TapeStore, ReadModelError> {
        validate_id(tape_id)?;
        let storage = self.storage_child("tapes")?;
        let tape_path = storage.join(tape_id);
        self.ensure_safe_path(&tape_path)?;
        reject_symlink_tree(&tape_path)?;
        // Check containment on the resolved path before passing it to shared I/O.
        // Keep the earlier no-symlink checks and valid single-component IDs.
        let tape_path = tape_path.canonicalize()?;
        if !tape_path.starts_with(&storage) {
            return Err(ReadModelError::UnsafePath);
        }
        let manifest_metadata = match fs::symlink_metadata(tape_path.join("manifest.json")) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ReadModelError::NotFound);
            }
            Err(_) => return Err(ReadModelError::InvalidDocument),
        };
        if !manifest_metadata.is_file() || manifest_metadata.len() > MAX_TAPE_MANIFEST_BYTES {
            return Err(ReadModelError::InvalidDocument);
        }
        TapeStore::open(&tape_path).map_err(map_tape_error)
    }

    pub fn tape_coverage(
        &self,
        tape_id: &str,
    ) -> Result<crate::coverage::TapeCoverage, ReadModelError> {
        let store = self.checked_tape(tape_id)?;
        let before = store.read_manifest().map_err(map_tape_error)?;
        let events = store
            .read_events_bounded(MAX_EVENT_BYTES, MAX_TAPE_EVENTS)
            .map_err(map_tape_error)?;
        let mut report =
            crate::coverage::summarize(tape_id, &before, events, MAX_EVENT_BYTES, MAX_TAPE_EVENTS)
                .map_err(map_tape_error)?;
        let after = store.read_manifest().map_err(map_tape_error)?;
        if before != after {
            report.state = "changed";
        }
        Ok(report)
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
            policy_plan: skilltape_verify::policy_plan(&package).ok(),
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

    pub fn receipt_context(&self, id: &str) -> Result<Value, ReadModelError> {
        validate_id(id)?;
        let root = self.storage_child("receipts")?;
        let receipt =
            self.bounded_stored_file(&root.join(format!("{id}.json")), MAX_STORED_DOCUMENT_BYTES)?;
        let review = |status: &str, context: Option<skilltape_verify::RunContext>| {
            serde_json::json!({
                "schema": "skilltape.dev/context-review/v1", "id": id, "status": status,
                "context": context, "provenance": "not-authenticated"
            })
        };
        let bytes = match self.bounded_stored_file(&root.join(format!("{id}.json.context")), 4096) {
            Ok(bytes) => bytes,
            Err(ReadModelError::NotFound) => return Ok(review("unrecorded", None)),
            Err(ReadModelError::InvalidDocument) => return Ok(review("invalid", None)),
            Err(error) => return Err(error),
        };
        match serde_json::from_slice::<skilltape_verify::RunContext>(&bytes) {
            Ok(context) if context.binds_receipt(&receipt) => Ok(review("recorded", Some(context))),
            _ => Ok(review("invalid", None)),
        }
    }

    pub fn batch_review(&self) -> Result<skilltape_core::batch::BatchReview, ReadModelError> {
        use skilltape_core::batch::{BatchPlan, BatchState, BatchStatus, MAX_BATCH_METADATA_BYTES};
        let metadata = self.storage_child("batch.json")?;
        let bytes = self.bounded_stored_file(&metadata, MAX_BATCH_METADATA_BYTES)?;
        let state: BatchState =
            serde_json::from_slice(&bytes).map_err(|_| ReadModelError::InvalidDocument)?;
        state
            .validate()
            .map_err(|_| ReadModelError::InvalidDocument)?;
        let plan = self.bounded_stored_file(
            &self.storage_child("batch-plan.json")?,
            MAX_BATCH_METADATA_BYTES,
        )?;
        let parsed: BatchPlan =
            serde_json::from_slice(&plan).map_err(|_| ReadModelError::InvalidDocument)?;
        parsed
            .validate()
            .map_err(|_| ReadModelError::InvalidDocument)?;
        if sha256(&plan) != state.plan_sha256
            || parsed.cases.len() != state.cases.len()
            || parsed
                .cases
                .iter()
                .zip(&state.cases)
                .any(|(plan, case)| plan.id != case.id)
        {
            return Err(ReadModelError::InvalidDocument);
        }
        let mut report = state.review();
        for (index, case) in state.cases.iter().enumerate() {
            let Some(attempt) = case.attempts.last() else {
                continue;
            };
            if !matches!(
                attempt.status,
                BatchStatus::Succeeded | BatchStatus::Failed | BatchStatus::Cancelled
            ) {
                continue;
            }
            let valid = (|| -> Result<bool, ReadModelError> {
                let id = format!("{}-a{}", case.id, attempt.number);
                validate_id(&id)?;
                let folder = self.storage_child("receipts")?;
                let receipt =
                    self.bounded_stored_file(&folder.join(format!("{id}.json")), 1024 * 1024)?;
                let context =
                    self.bounded_stored_file(&folder.join(format!("{id}.json.context")), 4096)?;
                let parsed: skilltape_verify::RunContext = serde_json::from_slice(&context)
                    .map_err(|_| ReadModelError::InvalidDocument)?;
                let document: Value = serde_json::from_slice(&receipt)
                    .map_err(|_| ReadModelError::InvalidDocument)?;
                let recorded_status = match document["status"].as_str() {
                    Some("succeeded") => BatchStatus::Succeeded,
                    Some("cancelled") => BatchStatus::Cancelled,
                    Some("run_failed" | "assertion_failed") => BatchStatus::Failed,
                    _ => return Ok(false),
                };
                Ok(
                    attempt.receipt_sha256.as_deref() == Some(sha256(&receipt).as_str())
                        && attempt.context_sha256.as_deref() == Some(sha256(&context).as_str())
                        && parsed.binds_receipt(&receipt)
                        && recorded_status == attempt.status
                        && parsed.skill_hash == state.package_sha256
                        && parsed.input_set_sha256 == attempt.input_sha256,
                )
            })()
            .unwrap_or(false);
            if !valid {
                report.status = "unconfirmed";
                report.cases[index].status = BatchStatus::Unconfirmed;
                report.cases[index].evidence = "changed";
                report.cases[index].error_code = Some("evidence-changed".into());
            }
        }
        Ok(report)
    }

    fn bounded_stored_file(&self, path: &Path, limit: u64) -> Result<Vec<u8>, ReadModelError> {
        self.ensure_safe_path(path)?;
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReadModelError::NotFound,
            _ => ReadModelError::UnsafePath,
        })?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > limit {
            return Err(ReadModelError::InvalidDocument);
        }
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(ReadModelError::InvalidDocument);
        }
        Ok(bytes)
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
        if fs::metadata(path.join("manifest.json"))?.len() > MAX_TAPE_MANIFEST_BYTES {
            return Err(ReadModelError::InvalidDocument);
        }
        let store = TapeStore::open(path).map_err(map_tape_error)?;
        let manifest = store.read_manifest().map_err(map_tape_error)?;
        Ok(TapeSummary {
            id: id.to_owned(),
            recorded_id: manifest.id.clone(),
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
        let bytes = self.bounded_stored_file(path, MAX_STORED_DOCUMENT_BYTES)?;
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
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
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
