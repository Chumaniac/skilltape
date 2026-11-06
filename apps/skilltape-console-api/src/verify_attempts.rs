//! Bounded inspection of content-free single Verify metadata; no execution/retry.
use super::{collection, normalize_page, Collection, ConsoleReadModel, ReadModelError};
use serde::Serialize;
use skilltape_verify::attempt::{
    valid_id, VerifyAttemptFinish, VerifyAttemptOutcome, VerifyAttemptReceipt, VerifyAttemptStart,
    MAX_VERIFY_ATTEMPTS, MAX_VERIFY_ATTEMPT_BYTES, MAX_VERIFY_ATTEMPT_ENTRIES,
};
use std::{collections::BTreeSet, fs, io, path::Path};

#[derive(Clone, Debug, Serialize)]
pub struct VerifyAttemptView {
    pub id: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub outcome: VerifyAttemptOutcome,
    pub receipt: Option<VerifyAttemptReceipt>,
    pub record_integrity: &'static str,
    pub read_only: bool,
    pub provenance: &'static str,
    pub saved_receipt_verified: bool,
}

impl ConsoleReadModel {
    fn attempt_metadata(&self, path: &Path) -> Result<Vec<u8>, ReadModelError> {
        let (bytes, before) =
            self.bounded_stored_file_info_checked(path, MAX_VERIFY_ATTEMPT_BYTES, |info| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    info.nlink() == 1
                }
                #[cfg(not(unix))]
                {
                    let _ = info;
                    true
                }
            })?;
        self.ensure_safe_path(path)?;
        let after = fs::symlink_metadata(path).map_err(|_| ReadModelError::UnsafePath)?;
        if !after.is_file()
            || after.file_type().is_symlink()
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
        {
            return Err(ReadModelError::UnsafePath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if before.dev() != after.dev()
                || before.ino() != after.ino()
                || before.ctime() != after.ctime()
                || before.ctime_nsec() != after.ctime_nsec()
            {
                return Err(ReadModelError::UnsafePath);
            }
        }
        Ok(bytes)
    }
    pub fn verify_attempt(&self, id: &str) -> Result<VerifyAttemptView, ReadModelError> {
        if !valid_id(id) {
            return Err(ReadModelError::UnsafeId);
        }
        let root = self.storage_child("attempts")?;
        let raw = self.attempt_metadata(&root.join(format!("{id}.started.json")))?;
        let start: VerifyAttemptStart =
            serde_json::from_slice(&raw).map_err(|_| ReadModelError::InvalidDocument)?;
        if !start.valid() || start.id != id {
            return Err(ReadModelError::InvalidDocument);
        }
        let mut result = VerifyAttemptView {
            id: id.into(),
            started_at_ms: start.started_at_ms,
            finished_at_ms: None,
            outcome: VerifyAttemptOutcome::Unconfirmed,
            receipt: None,
            record_integrity: "start-only",
            read_only: true,
            provenance: "not-authenticated",
            saved_receipt_verified: false,
        };
        match self.attempt_metadata(&root.join(format!("{id}.finished.json"))) {
            Ok(bytes) => {
                let finish: VerifyAttemptFinish =
                    serde_json::from_slice(&bytes).map_err(|_| ReadModelError::InvalidDocument)?;
                if !finish.matches_start(&start, &raw) {
                    return Err(ReadModelError::InvalidDocument);
                }
                result.finished_at_ms = Some(finish.finished_at_ms);
                result.outcome = finish.outcome;
                result.receipt = finish.receipt;
                result.record_integrity = "metadata-bound";
            }
            Err(ReadModelError::NotFound) => {}
            Err(error) => return Err(error),
        }
        Ok(result)
    }
    pub fn verify_attempts(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Collection<VerifyAttemptView>, ReadModelError> {
        let (_, limit) = normalize_page(Some(offset), Some(limit))?;
        let limit = limit.min(50);
        let root = self.storage_child("attempts")?;
        let entries = match fs::read_dir(root) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(collection(Vec::new(), offset, limit, 0))
            }
            Err(error) => return Err(error.into()),
        };
        let mut starts = BTreeSet::new();
        let mut finishes = BTreeSet::new();
        for (index, entry) in entries.enumerate() {
            if index >= MAX_VERIFY_ATTEMPT_ENTRIES {
                return Err(ReadModelError::AttemptCapacity);
            }
            let entry = entry?;
            if !entry.file_type()?.is_file() || entry.file_type()?.is_symlink() {
                return Err(ReadModelError::UnsafePath);
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ReadModelError::InvalidDocument)?;
            // Unpublished temporary files cannot establish an attempt or completion.
            if name == ".attempt.lock" || name.starts_with(".skilltape-receipt-") {
                continue;
            }
            if let Some(id) = name.strip_suffix(".started.json").filter(|id| valid_id(id)) {
                starts.insert(id.to_owned());
            } else if let Some(id) = name
                .strip_suffix(".finished.json")
                .filter(|id| valid_id(id))
            {
                finishes.insert(id.to_owned());
            } else {
                return Err(ReadModelError::InvalidDocument);
            }
        }
        if starts.len() > MAX_VERIFY_ATTEMPTS {
            return Err(ReadModelError::AttemptCapacity);
        }
        if !finishes.is_subset(&starts) {
            return Err(ReadModelError::InvalidDocument);
        }
        let mut rows = starts
            .iter()
            .map(|id| self.verify_attempt(id))
            .collect::<Result<Vec<_>, _>>()?;
        rows.sort_by(|a, b| {
            b.started_at_ms
                .cmp(&a.started_at_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        let total = rows.len();
        Ok(collection(
            rows.into_iter().skip(offset).take(limit).collect(),
            offset,
            limit,
            total,
        ))
    }
}
