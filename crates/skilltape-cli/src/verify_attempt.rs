//! Optional, bounded metadata journal. No command/input/output bodies are retained.
use super::super::batch_command::{self, BatchLock};
use super::{write_receipt, VerifyConfig};
use skilltape_verify::attempt::{
    digest, valid_id, VerifyAttemptFinish, VerifyAttemptOutcome, VerifyAttemptReceipt,
    VerifyAttemptStart, FINISH_SCHEMA, MAX_VERIFY_ATTEMPTS, MAX_VERIFY_ATTEMPT_BYTES,
    MAX_VERIFY_ATTEMPT_ENTRIES, START_SCHEMA,
};
use skilltape_verify::{Receipt, RunContext};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(super) struct AttemptJournal {
    root: PathBuf,
    guard: BatchLock,
    start: VerifyAttemptStart,
    bytes: Vec<u8>,
}
fn fail() -> io::Error {
    io::Error::other("attempt metadata is unavailable; no automatic retry")
}
fn now_ms() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| fail())?
        .as_millis()
        .try_into()
        .map_err(|_| fail())
}
fn destination(path: &Path) -> io::Result<PathBuf> {
    let path = batch_command::checked_path(path)?;
    if path.as_os_str().len() > 4096 {
        return Err(fail());
    }
    batch_command::safe(&path)?;
    let mut current = path.as_path();
    let mut suffix = Vec::new();
    while !current.try_exists()? {
        suffix.push(current.file_name().ok_or_else(fail)?.to_owned());
        current = current.parent().ok_or_else(fail)?;
    }
    let mut resolved = fs::canonicalize(current)?;
    for name in suffix.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}
fn private_directory(path: &Path) -> io::Result<()> {
    let info = fs::symlink_metadata(path)?;
    if !info.is_dir() || info.file_type().is_symlink() {
        return Err(fail());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if info.uid() != unsafe { libc::geteuid() } || info.mode() & 0o777 != 0o700 {
            return Err(fail());
        }
        let parent = fs::symlink_metadata(path.parent().ok_or_else(fail)?)?;
        if parent.uid() != unsafe { libc::geteuid() } || parent.mode() & 0o022 != 0 {
            return Err(fail());
        }
    }
    Ok(())
}
fn capacity(root: &Path) -> io::Result<()> {
    let mut starts = BTreeSet::new();
    let mut finishes = BTreeSet::new();
    for (index, entry) in fs::read_dir(root)?.enumerate() {
        if index >= MAX_VERIFY_ATTEMPT_ENTRIES {
            return Err(fail());
        }
        let entry = entry?;
        let info = fs::symlink_metadata(entry.path())?;
        if !info.is_file() || info.file_type().is_symlink() || info.len() > MAX_VERIFY_ATTEMPT_BYTES
        {
            return Err(fail());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if info.nlink() != 1
                || info.uid() != unsafe { libc::geteuid() }
                || !matches!(info.mode() & 0o777, 0o400 | 0o600)
            {
                return Err(fail());
            }
        }
        let name = entry.file_name().into_string().map_err(|_| fail())?;
        if name == ".attempt.lock" {
            if info.len() != 0 {
                return Err(fail());
            }
            continue;
        }
        let (id, start) = if let Some(id) = name.strip_suffix(".started.json") {
            (id, true)
        } else if let Some(id) = name.strip_suffix(".finished.json") {
            (id, false)
        } else {
            return Err(fail());
        };
        if !valid_id(id) {
            return Err(fail());
        }
        if start {
            starts.insert(id.to_owned());
        } else {
            finishes.insert(id.to_owned());
        }
    }
    if starts.len() >= MAX_VERIFY_ATTEMPTS || !finishes.is_subset(&starts) {
        return Err(fail());
    }
    for id in starts {
        let raw = batch_command::read(
            &root.join(format!("{id}.started.json")),
            MAX_VERIFY_ATTEMPT_BYTES,
        )?;
        let start: VerifyAttemptStart = serde_json::from_slice(&raw)?;
        if !start.valid() || start.id != id {
            return Err(fail());
        }
        if finishes.contains(&id) {
            let end = batch_command::read(
                &root.join(format!("{id}.finished.json")),
                MAX_VERIFY_ATTEMPT_BYTES,
            )?;
            let end: VerifyAttemptFinish = serde_json::from_slice(&end)?;
            if !end.matches_start(&start, &raw) {
                return Err(fail());
            }
        }
    }
    Ok(())
}
impl AttemptJournal {
    pub(super) fn begin(path: &Path, config: &VerifyConfig) -> io::Result<Self> {
        let selected = destination(path)?;
        for source in std::iter::once(&config.skill_path)
            .chain(config.input.iter())
            .chain(config.receipt.iter())
            .chain(config.context.iter())
            .chain(config.delivery_dir.iter())
        {
            let source = destination(source)?;
            if selected.starts_with(&source) || source.starts_with(&selected) {
                return Err(fail());
            }
        }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&selected)?;
        let root = batch_command::directory(&selected)?;
        private_directory(&root)?;
        let guard = batch_command::lock_named(&root, ".attempt.lock")?;
        guard.check()?;
        capacity(&root)?;
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| fail())?
            .as_nanos();
        let id = digest(
            format!(
                "{time}:{}:{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )
            .as_bytes(),
        );
        let start = VerifyAttemptStart {
            schema: START_SCHEMA.into(),
            id,
            started_at_ms: now_ms()?,
        };
        if !start.valid() {
            return Err(fail());
        }
        let bytes = serde_json::to_vec(&start)?;
        let journal = Self {
            root,
            guard,
            start,
            bytes,
        };
        journal.publish("started", &journal.bytes)?;
        eprintln!("Verify attempt: {}", journal.start.id);
        Ok(journal)
    }
    fn publish(&self, kind: &str, bytes: &[u8]) -> io::Result<()> {
        self.guard.check()?;
        private_directory(&self.root)?;
        if bytes.len() as u64 > MAX_VERIFY_ATTEMPT_BYTES {
            return Err(fail());
        }
        let path = self.root.join(format!("{}.{}.json", self.start.id, kind));
        let text = std::str::from_utf8(bytes).map_err(|_| fail())?;
        write_receipt(&path, text).map_err(|_| fail())?;
        #[cfg(unix)]
        fs::File::open(&self.root)?.sync_all()?;
        self.guard.check()
    }
    pub(super) fn finish(
        &self,
        receipt: Option<(&Receipt, &RunContext, &str)>,
        invoked: bool,
    ) -> io::Result<()> {
        self.guard.check()?;
        if batch_command::read(
            &self.root.join(format!("{}.started.json", self.start.id)),
            MAX_VERIFY_ATTEMPT_BYTES,
        )? != self.bytes
        {
            return Err(fail());
        }
        let (outcome, receipt) = if let Some((receipt, context, document)) = receipt {
            let hash = digest(document.as_bytes());
            if context.receipt_sha256 != hash
                || context.run_id != receipt.run_id
                || context.skill_hash != receipt.skill_hash
            {
                return Err(fail());
            }
            (
                VerifyAttemptOutcome::ReceiptProduced,
                Some(VerifyAttemptReceipt {
                    receipt_sha256: hash,
                    run_id: receipt.run_id.clone(),
                    skill_hash: receipt.skill_hash.clone(),
                    input_set_sha256: context.input_set_sha256.clone(),
                    status: receipt.status,
                }),
            )
        } else {
            (
                if invoked {
                    VerifyAttemptOutcome::Unconfirmed
                } else {
                    VerifyAttemptOutcome::PreparationFailed
                },
                None,
            )
        };
        let finish = VerifyAttemptFinish {
            schema: FINISH_SCHEMA.into(),
            id: self.start.id.clone(),
            start_sha256: digest(&self.bytes),
            finished_at_ms: now_ms()?.max(self.start.started_at_ms),
            outcome,
            receipt,
        };
        if !finish.matches_start(&self.start, &self.bytes) {
            return Err(fail());
        }
        self.publish("finished", &serde_json::to_vec(&finish)?)
    }
}
