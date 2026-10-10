//! Bounded, read-only inspection of actual CLI delivery directories.
use crate::read_model::{
    Collection, ConsoleReadModel, ReadModelError, CONSOLE_SCHEMA_V1, WORKSPACE_ID,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DeliverySummary {
    pub id: String,
    pub metadata_valid: bool,
    pub reported_status: Option<String>,
    pub run_id: Option<String>,
    pub declared_files: Option<usize>,
    pub declared_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeliveryFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeliveryFinding {
    pub code: &'static str,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeliveryReview {
    pub schema: &'static str,
    pub id: String,
    pub status: &'static str,
    pub reported_status: Option<String>,
    pub run_id: Option<String>,
    pub receipt_binding: &'static str,
    pub requirement_validation: &'static str,
    pub provenance: &'static str,
    pub declared_files: Option<usize>,
    pub declared_bytes: Option<u64>,
    pub checked_files: usize,
    pub checked_bytes: u64,
    pub files: Vec<DeliveryFile>,
    pub files_truncated: bool,
    pub findings: Vec<DeliveryFinding>,
    pub findings_truncated: bool,
}

#[cfg(unix)]
impl DeliveryReview {
    fn new(id: &str) -> Self {
        Self {
            schema: "skilltape.dev/delivery-review/v1",
            id: id.to_owned(),
            status: "failed",
            reported_status: None,
            run_id: None,
            receipt_binding: "failed",
            requirement_validation: "not-run",
            provenance: "not-authenticated",
            declared_files: None,
            declared_bytes: None,
            checked_files: 0,
            checked_bytes: 0,
            files: Vec::new(),
            files_truncated: false,
            findings: Vec::new(),
            findings_truncated: false,
        }
    }
    fn issue(&mut self, code: &'static str, path: Option<&str>) {
        if self.findings.len() < 64 {
            self.findings.push(DeliveryFinding {
                code,
                path: path.map(str::to_owned),
            });
        } else {
            self.findings_truncated = true;
        }
    }
}

impl ConsoleReadModel {
    pub fn deliveries(
        &self,
        workspace: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Collection<DeliverySummary>, ReadModelError> {
        if workspace != WORKSPACE_ID {
            return Err(ReadModelError::NotFound);
        }
        crate::read_model::normalize_page(Some(offset), Some(limit))?;
        #[cfg(unix)]
        {
            native::list(self.root(), offset, limit)
        }
        #[cfg(not(unix))]
        {
            let _ = (offset, limit);
            Err(ReadModelError::UnsupportedPlatform)
        }
    }
    pub fn delivery(&self, id: &str) -> Result<DeliveryReview, ReadModelError> {
        validate_delivery_id(id)?;
        #[cfg(unix)]
        {
            native::review(self.root(), id)
        }
        #[cfg(not(unix))]
        {
            Err(ReadModelError::UnsupportedPlatform)
        }
    }
}

pub(crate) fn validate_delivery_id(id: &str) -> Result<(), ReadModelError> {
    crate::read_model::validate_id(id)?;
    if id.chars().count() > 128
        || id.len() > 255
        || id.starts_with('.')
        || id.chars().any(char::is_control)
    {
        return Err(ReadModelError::UnsafeId);
    }
    Ok(())
}

#[cfg(unix)]
mod native {
    use super::*;
    use serde::de::{MapAccess, SeqAccess, Visitor};
    use serde::Deserialize;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use skilltape_schema::{validate_json, SchemaId};
    use std::collections::BTreeSet;
    use std::ffi::CString;
    use std::fs::{self, File, Metadata, OpenOptions};
    use std::io::{self, Read};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Component, Path};

    const METADATA_BYTES: u64 = 1024 * 1024;
    const FILE_BYTES: u64 = 16 * 1024 * 1024;
    const TOTAL_BYTES: u64 = 64 * 1024 * 1024;
    const ENTRIES: usize = 10_000;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Artifact {
        path: String,
        bytes: u64,
        sha256: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        schema: String,
        run_id: String,
        skill_hash: String,
        receipt_sha256: String,
        artifact_set_sha256: String,
        files: Vec<Artifact>,
    }
    struct Bound {
        manifest: Manifest,
        status: String,
        manifest_hash: String,
        receipt_hash: String,
    }
    type Identity = (u64, u64, u32, u64, i64, i64, i64, i64);

    fn identity(info: &Metadata) -> Identity {
        (
            info.dev(),
            info.ino(),
            info.mode(),
            info.len(),
            info.mtime(),
            info.mtime_nsec(),
            info.ctime(),
            info.ctime_nsec(),
        )
    }
    fn directory(info: &Metadata) -> (u64, u64) {
        (info.dev(), info.ino())
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }
    fn is_sha(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
    fn invalid() -> ReadModelError {
        ReadModelError::InvalidDocument
    }

    fn root(path: &Path) -> Result<File, ReadModelError> {
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| ReadModelError::UnsafePath)
    }
    fn open_member(owner: &File, relative: &Path, want_dir: bool) -> Result<File, ReadModelError> {
        let parts = relative.components().collect::<Vec<_>>();
        if parts.is_empty()
            || parts.len() > 65
            || parts
                .iter()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(ReadModelError::UnsafePath);
        }
        let mut parent = owner.try_clone().map_err(ReadModelError::Io)?;
        for (index, part) in parts.iter().enumerate() {
            let name = CString::new(part.as_os_str().as_bytes())
                .map_err(|_| ReadModelError::UnsafePath)?;
            let is_dir = index + 1 < parts.len() || want_dir;
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if is_dir { libc::O_DIRECTORY } else { 0 };
            // The returned descriptor is newly owned and cannot follow a component link.
            let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                let error = io::Error::last_os_error();
                return Err(if error.kind() == io::ErrorKind::NotFound {
                    ReadModelError::NotFound
                } else {
                    ReadModelError::UnsafePath
                });
            }
            parent = unsafe { File::from_raw_fd(fd) };
            let metadata = parent.metadata().map_err(ReadModelError::Io)?;
            if (is_dir && !metadata.is_dir()) || (!is_dir && !metadata.is_file()) {
                return Err(ReadModelError::UnsafePath);
            }
        }
        Ok(parent)
    }
    fn stable_root(path: &Path, anchored: &File) -> Result<(), ReadModelError> {
        let linked = fs::symlink_metadata(path).map_err(|_| invalid())?;
        if !linked.is_dir()
            || linked.file_type().is_symlink()
            || directory(&linked) != directory(&anchored.metadata().map_err(ReadModelError::Io)?)
        {
            return Err(invalid());
        }
        Ok(())
    }
    fn metadata(owner: &File, name: &str) -> Result<Vec<u8>, ReadModelError> {
        let mut file = open_member(owner, Path::new(name), false)?;
        let before = file.metadata().map_err(ReadModelError::Io)?;
        if before.len() > METADATA_BYTES {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(before.len() + 1)
            .read_to_end(&mut bytes)
            .map_err(ReadModelError::Io)?;
        let linked = open_member(owner, Path::new(name), false)?;
        if bytes.len() as u64 != before.len()
            || identity(&before) != identity(&file.metadata().map_err(ReadModelError::Io)?)
            || identity(&before) != identity(&linked.metadata().map_err(ReadModelError::Io)?)
        {
            return Err(invalid());
        }
        Ok(bytes)
    }

    // JSON metadata may not hide duplicate fields at any nesting level.
    struct StrictValue(Value);
    impl<'de> Deserialize<'de> for StrictValue {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Strict;
            impl<'de> Visitor<'de> for Strict {
                type Value = StrictValue;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("strict JSON")
                }
                fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                    Ok(StrictValue(Value::Bool(value)))
                }
                fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                    Ok(StrictValue(value.into()))
                }
                fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                    Ok(StrictValue(value.into()))
                }
                fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                    serde_json::Number::from_f64(value)
                        .map(|n| StrictValue(Value::Number(n)))
                        .ok_or_else(|| E::custom("invalid number"))
                }
                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                    Ok(StrictValue(value.into()))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                    Ok(StrictValue(Value::Null))
                }
                fn visit_seq<A: SeqAccess<'de>>(
                    self,
                    mut sequence: A,
                ) -> Result<Self::Value, A::Error> {
                    let mut items = Vec::new();
                    while let Some(StrictValue(value)) = sequence.next_element()? {
                        items.push(value);
                    }
                    Ok(StrictValue(Value::Array(items)))
                }
                fn visit_map<A: MapAccess<'de>>(
                    self,
                    mut entries: A,
                ) -> Result<Self::Value, A::Error> {
                    let mut values = serde_json::Map::new();
                    while let Some((key, StrictValue(value))) =
                        entries.next_entry::<String, StrictValue>()?
                    {
                        if values.insert(key, value).is_some() {
                            return Err(serde::de::Error::custom("duplicate JSON field"));
                        }
                    }
                    Ok(StrictValue(Value::Object(values)))
                }
            }
            deserializer.deserialize_any(Strict)
        }
    }
    fn allowed(path: &str) -> bool {
        if path.is_empty()
            || path.encode_utf16().count() > 1024
            || path.len() > 4096
            || path.chars().any(char::is_control)
            || path.contains(['\\', ':'])
        {
            return false;
        }
        let parts = path.split('/').collect::<Vec<_>>();
        if parts.len() > 64
            || parts
                .iter()
                .any(|part| part.is_empty() || *part == "." || *part == "..")
        {
            return false;
        }
        parts.iter().all(|part| {
            let lower = part.to_ascii_lowercase();
            !lower.starts_with(".env")
                && ![".git", ".ssh", ".aws", "secrets", "credentials"].contains(&lower.as_str())
                && ![".pem", ".key", ".p12", ".pfx"]
                    .iter()
                    .any(|suffix| lower.ends_with(suffix))
        })
    }
    fn bound(owner: &File) -> Result<Bound, ReadModelError> {
        let manifest_bytes = metadata(owner, "delivery.json")?;
        let StrictValue(value) = serde_json::from_slice(&manifest_bytes).map_err(|_| invalid())?;
        let manifest: Manifest = serde_json::from_value(value).map_err(|_| invalid())?;
        if manifest.schema != "skilltape.dev/delivery/v1"
            || ![
                &manifest.run_id,
                &manifest.skill_hash,
                &manifest.receipt_sha256,
                &manifest.artifact_set_sha256,
            ]
            .iter()
            .all(|s| is_sha(s))
            || manifest.files.len() > ENTRIES
        {
            return Err(invalid());
        }
        let mut total = 0u64;
        let mut previous: Option<&str> = None;
        let mut set = Sha256::new();
        for file in &manifest.files {
            if !allowed(&file.path)
                || !is_sha(&file.sha256)
                || file.bytes > FILE_BYTES
                || previous.is_some_and(|path| path.as_bytes() >= file.path.as_bytes())
            {
                return Err(invalid());
            }
            total = total.checked_add(file.bytes).ok_or_else(invalid)?;
            if total > TOTAL_BYTES {
                return Err(invalid());
            }
            set.update(file.path.as_bytes());
            set.update([0]);
            set.update(file.bytes.to_be_bytes());
            set.update(file.sha256.as_bytes());
            previous = Some(&file.path);
        }
        if hex(&set.finalize()) != manifest.artifact_set_sha256 {
            return Err(invalid());
        }
        let receipt_bytes = metadata(owner, "receipt.json")?;
        let StrictValue(receipt) = serde_json::from_slice(&receipt_bytes).map_err(|_| invalid())?;
        for section in ["steps", "assertions", "policy_decisions"] {
            if receipt[section]
                .as_array()
                .is_none_or(|items| items.len() > 4096)
            {
                return Err(invalid());
            }
        }
        if validate_json(SchemaId::ReceiptV1, &receipt).is_err()
            || receipt["run_id"].as_str() != Some(manifest.run_id.as_str())
            || receipt["skill_hash"].as_str() != Some(manifest.skill_hash.as_str())
            || sha(&receipt_bytes) != manifest.receipt_sha256
        {
            return Err(invalid());
        }
        Ok(Bound {
            status: receipt["status"].as_str().ok_or_else(invalid)?.to_owned(),
            manifest_hash: sha(&manifest_bytes),
            receipt_hash: sha(&receipt_bytes),
            manifest,
        })
    }
    pub(super) fn list(
        path: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<Collection<DeliverySummary>, ReadModelError> {
        let anchored = root(path)?;
        let mut candidates = Vec::new();
        for (seen, entry) in fs::read_dir(path).map_err(ReadModelError::Io)?.enumerate() {
            if seen >= 1000 {
                return Err(invalid());
            }
            let entry = entry.map_err(ReadModelError::Io)?;
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if validate_delivery_id(&id).is_err() {
                continue;
            }
            let Ok(child) = open_member(&anchored, Path::new(&id), true) else {
                continue;
            };
            let marker = ["delivery.json", "receipt.json"]
                .iter()
                .any(|name| open_member(&child, Path::new(name), false).is_ok());
            if marker {
                candidates.push(id);
            }
        }
        candidates.sort();
        let total = candidates.len();
        let mut items = Vec::new();
        for id in candidates.into_iter().skip(offset).take(limit) {
            let child = open_member(&anchored, Path::new(&id), true)?;
            let metadata = bound(&child).ok();
            items.push(DeliverySummary {
                id,
                metadata_valid: metadata.is_some(),
                reported_status: metadata.as_ref().map(|b| b.status.clone()),
                run_id: metadata.as_ref().map(|b| b.manifest.run_id.clone()),
                declared_files: metadata.as_ref().map(|b| b.manifest.files.len()),
                declared_bytes: metadata
                    .as_ref()
                    .map(|b| b.manifest.files.iter().map(|f| f.bytes).sum()),
            });
        }
        stable_root(path, &anchored)?;
        let next = offset.saturating_add(items.len());
        Ok(Collection {
            schema: CONSOLE_SCHEMA_V1,
            items,
            offset,
            limit,
            total,
            next_offset: (next < total).then_some(next),
        })
    }
    fn inventory(path: &Path) -> Result<BTreeSet<String>, ReadModelError> {
        let mut result = BTreeSet::new();
        let mut queue = vec![(path.to_owned(), 0)];
        let mut count = 0;
        while let Some((directory, depth)) = queue.pop() {
            if depth > 64 {
                return Err(invalid());
            }
            for entry in fs::read_dir(&directory).map_err(ReadModelError::Io)? {
                count += 1;
                if count > ENTRIES {
                    return Err(invalid());
                }
                let entry = entry.map_err(ReadModelError::Io)?;
                let info = fs::symlink_metadata(entry.path()).map_err(ReadModelError::Io)?;
                if info.file_type().is_symlink() {
                    return Err(ReadModelError::UnsafePath);
                }
                if info.is_dir() {
                    queue.push((entry.path(), depth + 1));
                } else if info.is_file() {
                    let name = entry
                        .path()
                        .strip_prefix(path)
                        .map_err(|_| invalid())?
                        .to_str()
                        .ok_or_else(invalid)?
                        .to_owned();
                    if !allowed(&name) {
                        return Err(ReadModelError::UnsafePath);
                    }
                    result.insert(name);
                } else {
                    return Err(ReadModelError::UnsafePath);
                }
            }
        }
        Ok(result)
    }
    fn digest(
        owner: &File,
        name: &str,
        expected_bytes: u64,
    ) -> Result<(String, Identity), ReadModelError> {
        let mut file = open_member(owner, Path::new(name), false)?;
        let before = file.metadata().map_err(ReadModelError::Io)?;
        if before.len() != expected_bytes || before.len() > FILE_BYTES {
            return Err(invalid());
        }
        let mut hash = Sha256::new();
        let mut left = expected_bytes;
        let mut buffer = [0u8; 64 * 1024];
        while left > 0 {
            let take = left.min(buffer.len() as u64) as usize;
            let length = file.read(&mut buffer[..take]).map_err(ReadModelError::Io)?;
            if length == 0 {
                return Err(invalid());
            }
            hash.update(&buffer[..length]);
            left -= length as u64;
        }
        if file.read(&mut buffer[..1]).map_err(ReadModelError::Io)? != 0 {
            return Err(invalid());
        }
        let linked = open_member(owner, Path::new(name), false)?;
        if identity(&before) != identity(&file.metadata().map_err(ReadModelError::Io)?)
            || identity(&before) != identity(&linked.metadata().map_err(ReadModelError::Io)?)
        {
            return Err(invalid());
        }
        Ok((hex(&hash.finalize()), identity(&before)))
    }
    pub(super) fn review(path: &Path, id: &str) -> Result<DeliveryReview, ReadModelError> {
        let anchored = root(path)?;
        let owner = open_member(&anchored, Path::new(id), true)?;
        let selected_path = path.join(id);
        let mut report = DeliveryReview::new(id);
        let mut run = || -> Result<(), ReadModelError> {
            let mut entries = BTreeSet::new();
            for entry in fs::read_dir(&selected_path).map_err(ReadModelError::Io)? {
                entries.insert(entry.map_err(ReadModelError::Io)?.file_name());
                if entries.len() > 3 {
                    report.issue("delivery.layout", None);
                    return Ok(());
                }
            }
            let expected = ["artifacts", "delivery.json", "receipt.json"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect::<BTreeSet<_>>();
            if entries != expected {
                report.issue("delivery.layout", None);
                return Ok(());
            }
            let bound = bound(&owner)?;
            report.reported_status = Some(bound.status);
            report.run_id = Some(bound.manifest.run_id.clone());
            report.receipt_binding = "verified";
            report.declared_files = Some(bound.manifest.files.len());
            report.declared_bytes = Some(bound.manifest.files.iter().map(|file| file.bytes).sum());
            let payload = open_member(&owner, Path::new("artifacts"), true)?;
            let names = inventory(&selected_path.join("artifacts"))?;
            if names
                != bound
                    .manifest
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect()
            {
                report.issue("delivery.file-set", None);
                return Ok(());
            }
            let mut snapshots = Vec::new();
            for file in &bound.manifest.files {
                match digest(&payload, &file.path, file.bytes) {
                    Ok((observed, snapshot)) if observed == file.sha256 => {
                        snapshots.push((file.path.as_str(), snapshot));
                        report.checked_files += 1;
                        report.checked_bytes += file.bytes;
                        if report.files.len() < 100 {
                            report.files.push(DeliveryFile {
                                path: file.path.clone(),
                                bytes: file.bytes,
                                sha256: observed,
                            });
                        } else {
                            report.files_truncated = true;
                        }
                    }
                    _ => report.issue("delivery.artifact-digest-or-snapshot", Some(&file.path)),
                }
            }
            for (name, original) in snapshots {
                match open_member(&payload, Path::new(name), false)
                    .and_then(|file| file.metadata().map_err(ReadModelError::Io))
                {
                    Ok(linked) if identity(&linked) == original => {}
                    _ => report.issue("delivery.artifact-changed", Some(name)),
                }
            }
            let linked_payload = open_member(&owner, Path::new("artifacts"), true)?;
            if directory(&linked_payload.metadata().map_err(ReadModelError::Io)?)
                != directory(&payload.metadata().map_err(ReadModelError::Io)?)
                || inventory(&selected_path.join("artifacts"))? != names
            {
                report.issue("delivery.artifact-set-changed", None);
            }
            if sha(&metadata(&owner, "delivery.json")?) != bound.manifest_hash
                || sha(&metadata(&owner, "receipt.json")?) != bound.receipt_hash
            {
                report.issue("delivery.metadata-changed", None);
            }
            let linked = open_member(&anchored, Path::new(id), true)?;
            if directory(&linked.metadata().map_err(ReadModelError::Io)?)
                != directory(&owner.metadata().map_err(ReadModelError::Io)?)
            {
                return Err(invalid());
            }
            stable_root(path, &anchored)?;
            if report.findings.is_empty() {
                report.status = "passed";
            }
            Ok(())
        };
        if run().is_err() {
            report.status = "failed";
            report.issue("delivery.invalid-or-unsafe-snapshot", None);
        }
        Ok(report)
    }
}
