//! Explicit, serial verification batches. Original inputs and successful attempts stay immutable.
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use skilltape_core::batch::*;
use skilltape_core::SkillPackage;
use skilltape_runner::{
    digest_input_tree, publish_directory_noreplace, snapshot_inputs, validate_output_root,
};
use skilltape_verify::{package_fingerprint, ReceiptStatus, RunContext};

use super::run_command::{self, VerifyConfig};

const INPUT_BYTES: u64 = 128 * 1024 * 1024;
const OUTPUT_ADMISSION_BYTES: u64 = 256 * 1024 * 1024;
const DELIVERY_RESERVE: u64 = 66 * 1024 * 1024;
const FILE_BYTES: u64 = 16 * 1024 * 1024;
const RECEIPT_BYTES: u64 = 1024 * 1024;
const STATE: &str = ".skilltape/batch.json";
const PLAN: &str = ".skilltape/batch-plan.json";
const SKILL: &str = "skills/batch-skill";

#[derive(Subcommand)]
pub(super) enum BatchCommand {
    /// Freeze a reviewed Skill and input sets into a new private workspace. Runs nothing.
    Prepare {
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Run only pending cases, reusing completed evidence. Requires --yes.
    Run {
        workspace: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly retry one known failed/cancelled case, optionally with a new input snapshot.
    Retry {
        workspace: PathBuf,
        #[arg(long = "case")]
        case_id: String,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Read current metadata and Receipt bindings without executing anything.
    Inspect {
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Reconcile interrupted cases from complete bound evidence; never reruns an unknown case.
    Reconcile {
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

fn fail(message: &'static str) -> io::Error {
    io::Error::other(message)
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
// Batch paths have a deliberately narrow spelling. Construct the path from the
// checked text, so every filesystem caller receives the validated value rather
// than the original CLI/descriptor path. Links are checked separately below.
fn checked_path(path: &Path) -> io::Result<PathBuf> {
    let text = path
        .to_str()
        .ok_or_else(|| fail("batch paths must be UTF-8"))?;
    if text.contains("..") || text.chars().any(char::is_control) {
        return Err(fail(
            "batch paths cannot contain parent sequences or control characters",
        ));
    }
    Ok(PathBuf::from(text))
}
fn safe(path: &Path) -> io::Result<()> {
    if !run_command::ancestors_are_safe(path) {
        return Err(fail("unsafe batch path or link"));
    }
    Ok(())
}
fn directory(path: &Path) -> io::Result<PathBuf> {
    let path = checked_path(path)?;
    safe(&path)?;
    let info = fs::symlink_metadata(&path)?;
    if !info.is_dir() || info.file_type().is_symlink() {
        return Err(fail("batch path must be a real directory"));
    }
    checked_path(&fs::canonicalize(&path)?)
}
// Descriptor members are resolved at the filesystem boundary, independently of
// schema validation. Canonicalization never grants permission to leave the root.
fn relative_member(base: &Path, value: &str) -> io::Result<PathBuf> {
    if value.is_empty()
        || value.contains("..")
        || value.contains('\\')
        || value.contains(':')
        || Path::new(value).is_absolute()
    {
        return Err(fail("batch member must be a safe relative path"));
    }
    let selected = base.join(value);
    safe(&selected)?;
    let resolved = selected.canonicalize()?;
    if !resolved.starts_with(base) {
        return Err(fail("batch member escaped its selected root"));
    }
    Ok(resolved)
}
fn single_component(value: &str) -> io::Result<&str> {
    if value.is_empty()
        || value.len() > 64
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
    {
        return Err(fail(
            "batch identifiers must be single safe path components",
        ));
    }
    Ok(value)
}
fn same(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev()
            || left.ino() != right.ino()
            || left.ctime() != right.ctime()
            || left.ctime_nsec() != right.ctime_nsec()
        {
            return false;
        }
    }
    left.is_file() == right.is_file()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}
fn opened(path: &Path, limit: u64) -> io::Result<(File, fs::Metadata)> {
    let path = checked_path(path)?;
    safe(&path)?;
    let before = fs::symlink_metadata(&path)?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > limit {
        return Err(fail("batch file is not a bounded regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !same(&before, &file.metadata()?) {
        return Err(fail("batch file changed before reading"));
    }
    Ok((file, before))
}
fn read(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let path = checked_path(path)?;
    let (mut file, before) = opened(&path, limit)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != before.len()
        || !same(&before, &file.metadata()?)
        || !same(&before, &fs::symlink_metadata(path)?)
    {
        return Err(fail("batch file changed during reading"));
    }
    Ok(bytes)
}
fn hash_file(path: &Path, limit: u64) -> io::Result<String> {
    let path = checked_path(path)?;
    let (mut file, before) = opened(&path, limit)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 8192];
    let mut count = 0;
    loop {
        let got = file.read(&mut buffer)?;
        if got == 0 {
            break;
        }
        count += got as u64;
        if count > before.len() {
            return Err(fail("batch file grew during reading"));
        }
        digest.update(&buffer[..got]);
    }
    if count != before.len()
        || !same(&before, &file.metadata()?)
        || !same(&before, &fs::symlink_metadata(path)?)
    {
        return Err(fail("batch file changed during reading"));
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
fn decode<T: DeserializeOwned>(path: &Path, limit: u64) -> io::Result<T> {
    serde_json::from_slice(&read(path, limit)?).map_err(|_| fail("invalid batch document"))
}
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = checked_path(path)?;
    safe(&path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn bytes<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > MAX_BATCH_METADATA_BYTES {
        return Err(fail("batch metadata capacity exceeded"));
    }
    Ok(bytes)
}
fn save(root: &Path, state: &mut BatchState, guard: &BatchLock) -> io::Result<()> {
    let root = checked_path(root)?;
    guard.check()?;
    safe(&root)?;
    state.revision = state
        .revision
        .checked_add(1)
        .ok_or_else(|| fail("batch revision capacity exceeded"))?;
    state.validate().map_err(fail)?;
    let path = root.join(STATE);
    let parent = path.parent().unwrap();
    safe(&path)?;
    if let Ok(info) = fs::symlink_metadata(&path) {
        if !info.is_file() || info.file_type().is_symlink() {
            return Err(fail("batch state was replaced"));
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes(state)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path).map_err(|error| error.error)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
fn load(root: &Path) -> io::Result<(BatchPlan, BatchState)> {
    let plan_bytes = read(&root.join(PLAN), MAX_BATCH_METADATA_BYTES)?;
    let plan: BatchPlan =
        serde_json::from_slice(&plan_bytes).map_err(|_| fail("invalid stored batch plan"))?;
    plan.validate().map_err(fail)?;
    let state: BatchState = decode(&root.join(STATE), MAX_BATCH_METADATA_BYTES)?;
    state.validate().map_err(fail)?;
    if state.plan_sha256 != sha(&plan_bytes)
        || plan.cases.len() != state.cases.len()
        || plan
            .cases
            .iter()
            .zip(&state.cases)
            .any(|(a, b)| a.id != b.id)
    {
        return Err(fail("stored plan or case binding changed"));
    }
    Ok((plan, state))
}
struct BatchLock {
    file: File,
    root: PathBuf,
    metadata: fs::Metadata,
}
impl BatchLock {
    fn check(&self) -> io::Result<()> {
        let root = checked_path(&self.root)?;
        safe(&root)?;
        let current = fs::symlink_metadata(&root)?;
        if !current.is_dir() || current.file_type().is_symlink() {
            return Err(fail("batch workspace was replaced"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if self.metadata.dev() != current.dev() || self.metadata.ino() != current.ino() {
                return Err(fail("batch workspace was replaced"));
            }
        }
        #[cfg(not(unix))]
        if self.metadata.created().ok() != current.created().ok() {
            return Err(fail("batch workspace was replaced"));
        }
        if !same(
            &self.file.metadata()?,
            &fs::symlink_metadata(root.join(".skilltape/batch.lock"))?,
        ) {
            return Err(fail("batch lock was replaced"));
        }
        Ok(())
    }
}
fn lock(root: &Path) -> io::Result<BatchLock> {
    let root = checked_path(root)?;
    let metadata = fs::symlink_metadata(&root)?;
    let path = root.join(".skilltape/batch.lock");
    safe(&path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(fail("invalid batch lock"));
    }
    file.try_lock()
        .map_err(|_| fail("another batch operation owns this workspace"))?;
    let guard = BatchLock {
        file,
        root: root.to_owned(),
        metadata,
    };
    guard.check()?;
    Ok(guard)
}
fn input_path(root: &Path, id: &str, generation: u8) -> io::Result<PathBuf> {
    let id = single_component(id)?;
    let parent = root.join(if generation == 0 {
        "inputs"
    } else {
        "retry-inputs"
    });
    let path = if generation == 0 {
        parent.join(id)
    } else {
        parent.join(format!("{id}-a{generation}"))
    };
    if !path.starts_with(&parent) {
        return Err(fail("input snapshot escaped its selected root"));
    }
    checked_path(&path)
}
fn attempt_id(id: &str, number: u8) -> io::Result<String> {
    let id = single_component(id)?;
    Ok(format!("{id}-a{number}"))
}
fn receipt_path(root: &Path, id: &str) -> io::Result<PathBuf> {
    let id = single_component(id)?;
    let parent = root.join(".skilltape/receipts");
    let path = parent.join(format!("{id}.json"));
    if !path.starts_with(&parent) {
        return Err(fail("Receipt escaped its selected root"));
    }
    checked_path(&path)
}
fn inventory(root: &Path, max_bytes: u64) -> io::Result<BTreeMap<String, u64>> {
    fn visit(
        root: &Path,
        path: &Path,
        depth: usize,
        entries: &mut usize,
        total: &mut u64,
        max: u64,
        files: &mut BTreeMap<String, u64>,
    ) -> io::Result<()> {
        let path = checked_path(path)?;
        if depth > 64 {
            return Err(fail("batch tree depth exceeded"));
        }
        let info = fs::symlink_metadata(&path)?;
        if info.file_type().is_symlink() {
            return Err(fail("batch tree contains a link"));
        }
        if info.is_dir() {
            for entry in fs::read_dir(&path)? {
                *entries += 1;
                if *entries > 10_000 {
                    return Err(fail("batch tree entry capacity exceeded"));
                }
                visit(root, &entry?.path(), depth + 1, entries, total, max, files)?;
            }
        } else if info.is_file() {
            *total = total
                .checked_add(info.len())
                .ok_or_else(|| fail("batch byte capacity exceeded"))?;
            if info.len() > FILE_BYTES || *total > max {
                return Err(fail("batch tree byte capacity exceeded"));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| fail("batch tree escaped its root"))?
                .to_str()
                .ok_or_else(|| fail("batch paths must be UTF-8"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            if relative.contains('\\') || relative.chars().any(char::is_control) {
                return Err(fail("unsafe batch file name"));
            }
            files.insert(relative, info.len());
        } else {
            return Err(fail("batch tree contains a special file"));
        }
        Ok(())
    }
    directory(root)?;
    let mut files = BTreeMap::new();
    visit(root, root, 0, &mut 0, &mut 0, max_bytes, &mut files)?;
    Ok(files)
}
fn input_budget(root: &Path) -> io::Result<()> {
    let mut total = 0u64;
    let mut files = 0;
    for path in [
        root.join("skills"),
        root.join("inputs"),
        root.join("retry-inputs"),
    ] {
        let entries = inventory(&path, INPUT_BYTES)?;
        files += entries.len();
        total += entries.values().sum::<u64>();
    }
    if total > INPUT_BYTES || files > 10_000 {
        return Err(fail("frozen batch materials exceed 128 MiB or 10000 files"));
    }
    Ok(())
}
fn materials(root: &Path, state: &BatchState) -> io::Result<()> {
    input_budget(root)?;
    let package =
        SkillPackage::load(root.join(SKILL)).map_err(|_| fail("frozen Skill cannot be loaded"))?;
    if package_fingerprint(&package)? != state.package_sha256 {
        return Err(fail("frozen Skill changed; create a new batch"));
    }
    for case in &state.cases {
        let path = input_path(root, &case.id, case.generation)?;
        if digest_input_tree(&path).map_err(|_| fail("frozen input is invalid"))?
            != case.input_sha256
        {
            return Err(fail(
                "frozen inputs changed; inspect originals before continuing",
            ));
        }
    }
    Ok(())
}
fn output_admission(root: &Path, state: &BatchState) -> io::Result<()> {
    let mut used = 0;
    for case in &state.cases {
        for attempt in &case.attempts {
            let path = root.join(attempt_id(&case.id, attempt.number)?);
            if path.try_exists()? {
                used += inventory(&path, OUTPUT_ADMISSION_BYTES)?
                    .values()
                    .sum::<u64>();
            }
        }
    }
    if used > OUTPUT_ADMISSION_BYTES - DELIVERY_RESERVE {
        return Err(fail(
            "batch output admission budget reached; successful originals are retained",
        ));
    }
    Ok(())
}

fn prepare(manifest: &Path, output: &Path) -> io::Result<()> {
    let manifest = checked_path(manifest)?;
    let output = checked_path(output)?;
    let plan: BatchPlan = decode(&manifest, MAX_BATCH_METADATA_BYTES)?;
    plan.validate().map_err(fail)?;
    let base = directory(
        manifest
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let package = directory(&relative_member(&base, &plan.skill)?)?;
    SkillPackage::load(&package).map_err(|_| fail("Skill package cannot be loaded"))?;
    safe(&output)?;
    if fs::symlink_metadata(&output).is_ok() {
        return Err(fail("batch output must be a new directory"));
    }
    let parent = directory(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let target = parent.join(
        output
            .file_name()
            .ok_or_else(|| fail("batch output needs a name"))?,
    );
    for case in &plan.cases {
        let input = directory(&relative_member(&base, &case.input)?)?;
        validate_output_root(&package, &input, &target)
            .map_err(|_| fail("batch output overlaps source materials"))?;
    }
    let stage = tempfile::Builder::new()
        .prefix(".skilltape-batch-")
        .tempdir_in(&parent)?;
    for name in ["skills", "inputs", "retry-inputs", ".skilltape/receipts"] {
        fs::create_dir_all(stage.path().join(name))?;
    }
    snapshot_inputs(&package, &stage.path().join(SKILL))?;
    let frozen = SkillPackage::load(stage.path().join(SKILL))
        .map_err(|_| fail("copied Skill is invalid"))?;
    let mut cases = Vec::new();
    for case in &plan.cases {
        let path = input_path(stage.path(), &case.id, 0)?;
        snapshot_inputs(&relative_member(&base, &case.input)?, &path)?;
        input_budget(stage.path())?;
        cases.push(BatchCaseState {
            id: case.id.clone(),
            input_sha256: digest_input_tree(&path).map_err(|_| fail("input digest failed"))?,
            generation: 0,
            attempts: Vec::new(),
        });
    }
    let plan_bytes = bytes(&plan)?;
    write_new(&stage.path().join(PLAN), &plan_bytes)?;
    let state = BatchState {
        schema: BATCH_STATE_SCHEMA.into(),
        plan_sha256: sha(&plan_bytes),
        package_sha256: package_fingerprint(&frozen)?,
        revision: 1,
        cases,
    };
    state.validate().map_err(fail)?;
    write_new(&stage.path().join(STATE), &bytes(&state)?)?;
    publish_directory_noreplace(stage.path(), &target)?;
    #[cfg(unix)]
    File::open(&parent)?.sync_all()?;
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    path: String,
    bytes: u64,
    sha256: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryManifest {
    schema: String,
    run_id: String,
    skill_hash: String,
    receipt_sha256: String,
    artifact_set_sha256: String,
    files: Vec<ManifestFile>,
}

fn delivery_hash(
    root: &Path,
    id: &str,
    receipt: &Value,
    receipt_hash: &str,
    physical: bool,
) -> io::Result<(String, String)> {
    let path = root.join(id);
    let manifest_bytes = read(&path.join("delivery.json"), RECEIPT_BYTES)?;
    let manifest: DeliveryManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| fail("invalid delivery manifest"))?;
    if manifest.schema != "skilltape.dev/delivery/v1"
        || manifest.receipt_sha256 != receipt_hash
        || manifest.run_id != receipt["run_id"]
        || manifest.skill_hash != receipt["skill_hash"]
        || manifest.files.len() > 10_000
    {
        return Err(fail("delivery does not bind the case Receipt"));
    }
    if sha(&read(&path.join("receipt.json"), RECEIPT_BYTES)?) != receipt_hash {
        return Err(fail("delivery Receipt changed"));
    }
    let mut digest = Sha256::new();
    let mut declared = BTreeMap::new();
    let mut total = 0u64;
    for file in &manifest.files {
        if !safe_relative(&file.path)
            || !batch_sha(&file.sha256)
            || file.bytes > FILE_BYTES
            || declared.insert(file.path.clone(), file.bytes).is_some()
        {
            return Err(fail("invalid delivery file declaration"));
        }
        total = total
            .checked_add(file.bytes)
            .ok_or_else(|| fail("delivery byte capacity exceeded"))?;
        if total > 64 * 1024 * 1024 {
            return Err(fail("delivery byte capacity exceeded"));
        }
    }
    // Match the existing delivery inventory digest in sorted path order.
    let by_path: BTreeMap<_, _> = manifest
        .files
        .iter()
        .map(|file| (&file.path, file))
        .collect();
    for (name, file) in by_path {
        if physical
            && hash_file(&relative_member(&path.join("artifacts"), name)?, FILE_BYTES)?
                != file.sha256
        {
            return Err(fail("retained artifact changed"));
        }
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(file.bytes.to_be_bytes());
        digest.update(file.sha256.as_bytes());
    }
    let observed: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if observed != manifest.artifact_set_sha256
        || (physical && inventory(&path.join("artifacts"), 64 * 1024 * 1024)? != declared)
    {
        return Err(fail("retained delivery inventory changed"));
    }
    Ok((sha(&manifest_bytes), observed))
}
fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.contains(['\\', ':'])
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn audit(
    root: &Path,
    state: &BatchState,
    case_id: &str,
    attempt: &BatchAttempt,
    physical: bool,
) -> io::Result<BatchAttempt> {
    let id = attempt_id(case_id, attempt.number)?;
    let path = receipt_path(root, &id)?;
    let raw = read(&path, RECEIPT_BYTES)?;
    let receipt: Value = serde_json::from_slice(&raw).map_err(|_| fail("invalid batch Receipt"))?;
    let context_bytes = read(&path.with_extension("json.context"), 4096)?;
    let context: RunContext =
        serde_json::from_slice(&context_bytes).map_err(|_| fail("invalid run context"))?;
    if !context.binds_receipt(&raw)
        || context.skill_hash != state.package_sha256
        || context.input_set_sha256 != attempt.input_sha256
    {
        return Err(fail("Receipt or context no longer binds this case"));
    }
    let status = match receipt["status"].as_str() {
        Some("succeeded") => BatchStatus::Succeeded,
        Some("cancelled") => BatchStatus::Cancelled,
        Some("run_failed" | "assertion_failed") => BatchStatus::Failed,
        _ => return Err(fail("unsupported case outcome")),
    };
    let mut found = attempt.clone();
    found.status = status;
    found.receipt_sha256 = Some(sha(&raw));
    found.context_sha256 = Some(sha(&context_bytes));
    found.error_code = None;
    if status == BatchStatus::Succeeded {
        let (manifest, artifacts) = delivery_hash(
            root,
            &id,
            &receipt,
            found.receipt_sha256.as_deref().unwrap(),
            physical,
        )?;
        found.delivery_manifest_sha256 = Some(manifest);
        found.artifact_tree_sha256 = Some(artifacts);
    }
    for (old, new) in [
        (&attempt.receipt_sha256, &found.receipt_sha256),
        (&attempt.context_sha256, &found.context_sha256),
        (
            &attempt.delivery_manifest_sha256,
            &found.delivery_manifest_sha256,
        ),
        (&attempt.artifact_tree_sha256, &found.artifact_tree_sha256),
    ] {
        if old
            .as_ref()
            .is_some_and(|value| Some(value) != new.as_ref())
        {
            return Err(fail("saved evidence changed"));
        }
    }
    if matches!(
        attempt.status,
        BatchStatus::Succeeded | BatchStatus::Failed | BatchStatus::Cancelled
    ) && attempt.status != found.status
    {
        return Err(fail("saved outcome changed"));
    }
    Ok(found)
}
fn reconcile(root: &Path, state: &mut BatchState, guard: &BatchLock) -> io::Result<()> {
    for index in 0..state.cases.len() {
        let Some(last) = state.cases[index].attempts.last().cloned() else {
            continue;
        };
        let checked = audit(root, state, &state.cases[index].id, &last, true);
        if matches!(last.status, BatchStatus::Running | BatchStatus::Unconfirmed) {
            match checked {
                Ok(value) => {
                    *state.cases[index].attempts.last_mut().unwrap() = value;
                    save(root, state, guard)?;
                }
                Err(_) => {
                    let attempt = state.cases[index].attempts.last_mut().unwrap();
                    attempt.status = BatchStatus::Unconfirmed;
                    attempt.error_code = Some("execution-unconfirmed".into());
                    save(root, state, guard)?;
                    return Err(fail("unconfirmed case: inspect retained evidence; it will not run again automatically"));
                }
            }
        } else {
            checked?;
        }
    }
    Ok(())
}
fn execute(
    root: &Path,
    plan: &BatchPlan,
    state: &mut BatchState,
    index: usize,
    guard: &BatchLock,
) -> io::Result<bool> {
    guard.check()?;
    output_admission(root, state)?;
    let case = state.cases[index].clone();
    let number = (case.attempts.len() + 1) as u8;
    if number as usize > MAX_BATCH_ATTEMPTS {
        return Err(fail("case attempt capacity reached"));
    }
    let id = attempt_id(&case.id, number)?;
    let receipt = receipt_path(root, &id)?;
    let context = receipt.with_extension("json.context");
    if receipt.try_exists()? || context.try_exists()? || root.join(&id).try_exists()? {
        return Err(fail(
            "attempt output already exists; inspect it before continuing",
        ));
    }
    state.cases[index].attempts.push(BatchAttempt {
        number,
        status: BatchStatus::Running,
        input_sha256: case.input_sha256.clone(),
        generation: case.generation,
        receipt_sha256: None,
        context_sha256: None,
        delivery_manifest_sha256: None,
        artifact_tree_sha256: None,
        error_code: None,
    });
    save(root, state, guard)?;
    let request = &plan.cases[index];
    guard.check()?;
    let result = run_command::execute_verify(VerifyConfig {
        skill_path: root.join(SKILL),
        input: Some(input_path(root, &case.id, case.generation)?),
        receipt: None,
        context: None,
        delivery_dir: Some(root.join(&id)),
        expected_files: request.expected_files.clone(),
        expected_hashes: request.expected_hashes.clone(),
        json: false,
    });
    if let Ok((value, metadata)) = result {
        guard.check()?;
        let cancelled = value.status == ReceiptStatus::Cancelled;
        run_command::write_receipt(&receipt, &serde_json::to_string(&value)?)
            .map_err(|_| fail("could not save case Receipt; inspect retained evidence"))?;
        run_command::write_receipt(&context, &serde_json::to_string(&metadata)?)
            .map_err(|_| fail("could not save run context; inspect retained evidence"))?;
        let last = state.cases[index].attempts.last().unwrap();
        let checked = audit(root, state, &case.id, last, true)?;
        *state.cases[index].attempts.last_mut().unwrap() = checked;
        save(root, state, guard)?;
        return Ok(cancelled);
    }
    let attempt = state.cases[index].attempts.last_mut().unwrap();
    attempt.status = BatchStatus::Unconfirmed;
    attempt.error_code = Some("execution-unconfirmed".into());
    save(root, state, guard)?;
    Err(fail(
        "execution outcome unavailable: inspect this case before continuing",
    ))
}
fn retry_input(root: &Path, state: &mut BatchState, index: usize, input: &Path) -> io::Result<()> {
    let input = directory(input)?;
    let number = (state.cases[index].attempts.len() + 1) as u8;
    if input.starts_with(root) || root.starts_with(&input) {
        return Err(fail("retry inputs must be outside the batch workspace"));
    }
    let id = state.cases[index].id.clone();
    let target = input_path(root, &id, number)?;
    let digest = digest_input_tree(&input).map_err(|_| fail("invalid retry inputs"))?;
    if target.try_exists()? {
        if digest_input_tree(&target).map_err(|_| fail("invalid retained retry snapshot"))?
            != digest
        {
            return Err(fail(
                "retained retry preparation differs; preserve it and inspect before continuing",
            ));
        }
    } else {
        let stage = tempfile::Builder::new()
            .prefix(".batch-input-")
            .tempdir_in(root.join("retry-inputs"))?;
        let copy = stage.path().join("materials");
        snapshot_inputs(&input, &copy)?;
        input_budget(root)?;
        if digest_input_tree(&copy).map_err(|_| fail("retry input digest failed"))? != digest {
            return Err(fail("retry input changed during preparation"));
        }
        publish_directory_noreplace(&copy, &target)?;
    }
    input_budget(root)?;
    state.cases[index].generation = number;
    state.cases[index].input_sha256 = digest;
    Ok(())
}
fn review(root: &Path, physical: bool) -> io::Result<BatchReview> {
    let (_, state) = load(root)?;
    let mut report = state.review();
    for (index, case) in state.cases.iter().enumerate() {
        if let Some(attempt) = case.attempts.last() {
            if matches!(
                attempt.status,
                BatchStatus::Succeeded | BatchStatus::Failed | BatchStatus::Cancelled
            ) && audit(root, &state, &case.id, attempt, physical).is_err()
            {
                report.cases[index].status = BatchStatus::Unconfirmed;
                report.cases[index].evidence = "changed";
                report.cases[index].error_code = Some("evidence-changed".into());
                report.status = "unconfirmed";
            }
        }
    }
    Ok(report)
}

pub(super) fn run(command: BatchCommand) -> ExitCode {
    let result = (|| -> io::Result<(BatchReview, u8, bool)> {
        match command {
            BatchCommand::Prepare {
                manifest,
                output,
                json,
            } => {
                prepare(&manifest, &output)?;
                let root = directory(&output)?;
                Ok((review(&root, false)?, 0, json))
            }
            BatchCommand::Inspect { workspace, json } => {
                let root = directory(&workspace)?;
                Ok((review(&root, false)?, 0, json))
            }
            BatchCommand::Reconcile { workspace, json } => {
                let root = directory(&workspace)?;
                let guard = lock(&root)?;
                let (_, mut state) = load(&root)?;
                materials(&root, &state)?;
                reconcile(&root, &mut state, &guard)?;
                Ok((review(&root, false)?, 0, json))
            }
            BatchCommand::Run {
                workspace,
                yes,
                json,
            } => {
                if !yes {
                    return Err(fail("batch execution requires --yes"));
                }
                let root = directory(&workspace)?;
                let guard = lock(&root)?;
                let (plan, mut state) = load(&root)?;
                materials(&root, &state)?;
                reconcile(&root, &mut state, &guard)?;
                for index in 0..state.cases.len() {
                    if crate::interrupt::is_interrupted() {
                        return Ok((review(&root, false)?, 5, json));
                    }
                    if state.cases[index].attempts.is_empty()
                        && execute(&root, &plan, &mut state, index, &guard)?
                    {
                        return Ok((review(&root, false)?, 5, json));
                    }
                }
                let report = review(&root, false)?;
                let code = if report.status == "complete" { 0 } else { 3 };
                Ok((report, code, json))
            }
            BatchCommand::Retry {
                workspace,
                case_id,
                input,
                yes,
                json,
            } => {
                if !yes {
                    return Err(fail("retry execution requires --yes"));
                }
                let root = directory(&workspace)?;
                let guard = lock(&root)?;
                let (plan, mut state) = load(&root)?;
                materials(&root, &state)?;
                reconcile(&root, &mut state, &guard)?;
                let index = state
                    .cases
                    .iter()
                    .position(|case| case.id == case_id)
                    .ok_or_else(|| fail("case does not exist"))?;
                let last = state.cases[index]
                    .attempts
                    .last()
                    .ok_or_else(|| fail("case has not run; use batch run"))?;
                if !matches!(last.status, BatchStatus::Failed | BatchStatus::Cancelled)
                    || state.cases[index].attempts.len() >= MAX_BATCH_ATTEMPTS
                {
                    return Err(fail(
                        "only known failed or cancelled cases with remaining attempts may retry",
                    ));
                }
                if let Some(input) = input {
                    retry_input(&root, &mut state, index, &input)?;
                }
                let cancelled = execute(&root, &plan, &mut state, index, &guard)?;
                let report = review(&root, false)?;
                let code = if cancelled {
                    5
                } else if report.status == "complete" {
                    0
                } else {
                    3
                };
                Ok((report, code, json))
            }
        }
    })();
    match result {
        Ok((report, code, json)) => {
            if json {
                match serde_json::to_string(&report) {
                    Ok(value) => println!("{value}"),
                    Err(_) => return ExitCode::from(4),
                }
            } else {
                println!("Batch {} · {} cases. Open Console with this batch workspace to review saved evidence.",report.status,report.cases.len());
            }
            ExitCode::from(code)
        }
        Err(error) => {
            eprintln!("Batch: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn descriptor_members_and_components_cannot_escape_a_selected_root() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        fs::create_dir(base.join("allowed")).unwrap();
        fs::write(base.join("allowed/data.txt"), b"synthetic").unwrap();
        assert_eq!(
            relative_member(&base, "allowed/data.txt").unwrap(),
            base.join("allowed/data.txt")
        );
        for value in [
            "../outside",
            "allowed/../../outside",
            "/etc/passwd",
            "allowed\\outside",
            "a..b",
            "C:outside",
        ] {
            assert!(relative_member(&base, value).is_err());
        }
        for value in ["../outside", "a/b", "a\\b", "a..b", "a:b"] {
            assert!(single_component(value).is_err());
        }
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), base.join("linked")).unwrap();
        assert!(relative_member(&base, "linked").is_err());
    }
    #[test]
    fn replaced_workspace_cannot_receive_a_journal_update() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".skilltape")).unwrap();
        let state = BatchState {
            schema: BATCH_STATE_SCHEMA.into(),
            plan_sha256: "a".repeat(64),
            package_sha256: "b".repeat(64),
            revision: 1,
            cases: vec![BatchCaseState {
                id: "one".into(),
                input_sha256: "c".repeat(64),
                generation: 0,
                attempts: Vec::new(),
            }],
        };
        write_new(&root.join(STATE), &bytes(&state).unwrap()).unwrap();
        let guard = lock(&root).unwrap();
        fs::rename(&root, temp.path().join("retained")).unwrap();
        fs::create_dir_all(root.join(".skilltape")).unwrap();
        let original = bytes(&state).unwrap();
        fs::write(root.join(STATE), &original).unwrap();
        fs::write(root.join(".skilltape/batch.lock"), b"").unwrap();
        let mut next = state;
        assert!(save(&root, &mut next, &guard).is_err());
        assert_eq!(fs::read(root.join(STATE)).unwrap(), original);
    }
}
