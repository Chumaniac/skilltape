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

// Only filesystem discovery can construct work items. Request strings select
// existing entries in memory and never become paths inside blocking work.
pub(crate) struct DeliveryCatalog {
    #[cfg(unix)]
    inner: native::Catalog,
    #[cfg(not(unix))]
    unavailable: std::convert::Infallible,
}
pub(crate) struct DeliveryPageWork {
    #[cfg(unix)]
    inner: native::PageWork,
    #[cfg(not(unix))]
    unavailable: std::convert::Infallible,
}
pub(crate) struct DeliveryReviewWork {
    #[cfg(unix)]
    inner: native::ReviewWork,
    #[cfg(not(unix))]
    unavailable: std::convert::Infallible,
}
impl DeliveryCatalog {
    pub(crate) fn total(&self) -> usize {
        #[cfg(unix)]
        {
            self.inner.total()
        }
        #[cfg(not(unix))]
        {
            match self.unavailable {}
        }
    }
    pub(crate) fn page(
        self,
        offset: usize,
        limit: usize,
    ) -> Result<DeliveryPageWork, ReadModelError> {
        crate::read_model::normalize_page(Some(offset), Some(limit))?;
        #[cfg(unix)]
        {
            Ok(DeliveryPageWork {
                inner: self.inner.page(offset, limit),
            })
        }
        #[cfg(not(unix))]
        {
            match self.unavailable {}
        }
    }
    pub(crate) fn select(self, id: &str) -> Result<DeliveryReviewWork, ReadModelError> {
        validate_delivery_id(id)?;
        #[cfg(unix)]
        {
            Ok(DeliveryReviewWork {
                inner: self.inner.select(id)?,
            })
        }
        #[cfg(not(unix))]
        {
            match self.unavailable {}
        }
    }
}
impl DeliveryPageWork {
    pub(crate) fn read(self) -> Result<Vec<DeliverySummary>, ReadModelError> {
        #[cfg(unix)]
        {
            native::summaries(self.inner)
        }
        #[cfg(not(unix))]
        {
            match self.unavailable {}
        }
    }
}
impl DeliveryReviewWork {
    pub(crate) fn read(self) -> Result<DeliveryReview, ReadModelError> {
        #[cfg(unix)]
        {
            native::review(self.inner)
        }
        #[cfg(not(unix))]
        {
            match self.unavailable {}
        }
    }
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
    pub(crate) fn delivery_catalog(&self) -> Result<DeliveryCatalog, ReadModelError> {
        #[cfg(unix)]
        {
            Ok(DeliveryCatalog {
                inner: native::discover(self.root())?,
            })
        }
        #[cfg(not(unix))]
        {
            Err(ReadModelError::UnsupportedPlatform)
        }
    }
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
        let catalog = self.delivery_catalog()?;
        let total = catalog.total();
        let items = catalog.page(offset, limit)?.read()?;
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
    pub fn delivery(&self, id: &str) -> Result<DeliveryReview, ReadModelError> {
        validate_delivery_id(id)?;
        self.delivery_catalog()?.select(id)?.read()
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
    use std::ffi::{CStr, CString, OsStr, OsString};
    use std::fs::{self, File, Metadata, OpenOptions};
    use std::io::{self, Read};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Component, Path, PathBuf};

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

    struct Anchor {
        path: PathBuf,
        file: File,
    }
    struct Candidate {
        id: String,
        directory: (u64, u64),
    }
    pub(super) struct Catalog {
        anchor: Anchor,
        candidates: Vec<Candidate>,
    }
    pub(super) struct PageWork {
        anchor: Anchor,
        candidates: Vec<Candidate>,
    }
    pub(super) struct ReviewWork {
        anchor: Anchor,
        candidate: Candidate,
    }
    impl Catalog {
        pub(super) fn total(&self) -> usize {
            self.candidates.len()
        }
        pub(super) fn page(self, offset: usize, limit: usize) -> PageWork {
            let mut candidates = Vec::new();
            for (index, candidate) in self.candidates.into_iter().enumerate() {
                if index >= offset && candidates.len() < limit {
                    candidates.push(candidate);
                }
            }
            PageWork {
                anchor: self.anchor,
                candidates,
            }
        }
        pub(super) fn select(self, id: &str) -> Result<ReviewWork, ReadModelError> {
            for candidate in self.candidates {
                if candidate.id == id {
                    return Ok(ReviewWork {
                        anchor: self.anchor,
                        candidate,
                    });
                }
            }
            Err(ReadModelError::NotFound)
        }
    }

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
    fn open_component(owner: &File, name: &OsStr) -> Result<File, ReadModelError> {
        let name = CString::new(name.as_bytes()).map_err(|_| ReadModelError::UnsafePath)?;
        let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;
        // The descriptor owns the actual entry; links and special files are rejected.
        let fd = unsafe { libc::openat(owner.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return Err(if error.kind() == io::ErrorKind::NotFound {
                ReadModelError::NotFound
            } else {
                ReadModelError::UnsafePath
            });
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let info = file.metadata().map_err(ReadModelError::Io)?;
        if !info.is_file() && !info.is_dir() {
            return Err(ReadModelError::UnsafePath);
        }
        Ok(file)
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
            let is_dir = index + 1 < parts.len() || want_dir;
            parent = open_component(&parent, part.as_os_str())?;
            let metadata = parent.metadata().map_err(ReadModelError::Io)?;
            if (is_dir && !metadata.is_dir()) || (!is_dir && !metadata.is_file()) {
                return Err(ReadModelError::UnsafePath);
            }
        }
        Ok(parent)
    }
    fn entries(owner: &File, limit: usize) -> Result<Vec<OsString>, ReadModelError> {
        // Reopen the fixed "." member to get an independent directory offset.
        // dup() would share offsets and could make a later recheck appear empty.
        let fd = unsafe {
            libc::openat(
                owner.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(ReadModelError::Io(io::Error::last_os_error()));
        }
        let pointer = unsafe { libc::fdopendir(fd) };
        if pointer.is_null() {
            let error = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(ReadModelError::Io(error));
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        let stream = Stream(pointer);
        let mut names = Vec::new();
        loop {
            errno::set_errno(errno::Errno(0));
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = errno::errno().0;
                return if error == 0 {
                    Ok(names)
                } else {
                    Err(ReadModelError::Io(io::Error::from_raw_os_error(error)))
                };
            }
            // POSIX readdir owns this NUL-terminated name until the next call.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            if names.len() >= limit {
                return Err(invalid());
            }
            names.push(OsString::from_vec(name.to_owned()));
        }
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
    pub(super) fn discover(path: &Path) -> Result<Catalog, ReadModelError> {
        let anchored = root(path)?;
        let mut candidates = Vec::new();
        for entry in entries(&anchored, 1000)? {
            let Some(id) = entry.to_str().map(str::to_owned) else {
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
                candidates.push(Candidate {
                    id,
                    directory: directory(&child.metadata().map_err(ReadModelError::Io)?),
                });
            }
        }
        candidates.sort_by(|left, right| left.id.cmp(&right.id));
        stable_root(path, &anchored)?;
        Ok(Catalog {
            anchor: Anchor {
                path: path.to_owned(),
                file: anchored,
            },
            candidates,
        })
    }
    pub(super) fn summaries(work: PageWork) -> Result<Vec<DeliverySummary>, ReadModelError> {
        let PageWork { anchor, candidates } = work;
        stable_root(&anchor.path, &anchor.file)?;
        let mut items = Vec::new();
        for candidate in candidates {
            let child = open_member(&anchor.file, Path::new(&candidate.id), true)?;
            if directory(&child.metadata().map_err(ReadModelError::Io)?) != candidate.directory {
                return Err(invalid());
            }
            let metadata = bound(&child).ok();
            items.push(DeliverySummary {
                id: candidate.id,
                metadata_valid: metadata.is_some(),
                reported_status: metadata.as_ref().map(|b| b.status.clone()),
                run_id: metadata.as_ref().map(|b| b.manifest.run_id.clone()),
                declared_files: metadata.as_ref().map(|b| b.manifest.files.len()),
                declared_bytes: metadata
                    .as_ref()
                    .map(|b| b.manifest.files.iter().map(|f| f.bytes).sum()),
            });
        }
        stable_root(&anchor.path, &anchor.file)?;
        Ok(items)
    }
    fn inventory(owner: &File) -> Result<BTreeSet<String>, ReadModelError> {
        fn walk(
            owner: &File,
            prefix: &Path,
            depth: usize,
            count: &mut usize,
            result: &mut BTreeSet<String>,
        ) -> Result<(), ReadModelError> {
            if depth > 64 {
                return Err(invalid());
            }
            let names = entries(owner, ENTRIES - *count)?;
            // Charge discovery before recursion, including not-yet-visited siblings.
            *count += names.len();
            for entry in names {
                let relative = prefix.join(&entry);
                let name = relative.to_str().ok_or_else(invalid)?;
                if !allowed(name) {
                    return Err(ReadModelError::UnsafePath);
                }
                let child = open_component(owner, &entry)?;
                let info = child.metadata().map_err(ReadModelError::Io)?;
                if info.is_dir() {
                    // Depth-first ownership retains at most one directory per level.
                    walk(&child, &relative, depth + 1, count, result)?;
                } else {
                    result.insert(name.to_owned());
                }
            }
            Ok(())
        }
        let mut result = BTreeSet::new();
        walk(owner, Path::new(""), 0, &mut 0, &mut result)?;
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
    pub(super) fn review(work: ReviewWork) -> Result<DeliveryReview, ReadModelError> {
        let ReviewWork { anchor, candidate } = work;
        stable_root(&anchor.path, &anchor.file)?;
        let owner = open_member(&anchor.file, Path::new(&candidate.id), true)?;
        if directory(&owner.metadata().map_err(ReadModelError::Io)?) != candidate.directory {
            return Err(invalid());
        }
        let mut report = DeliveryReview::new(&candidate.id);
        let mut run = || -> Result<(), ReadModelError> {
            let names = entries(&owner, 3)?.into_iter().collect::<BTreeSet<_>>();
            let expected = ["artifacts", "delivery.json", "receipt.json"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect::<BTreeSet<_>>();
            if names != expected {
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
            let names = inventory(&payload)?;
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
                || inventory(&linked_payload)? != names
            {
                report.issue("delivery.artifact-set-changed", None);
            }
            if sha(&metadata(&owner, "delivery.json")?) != bound.manifest_hash
                || sha(&metadata(&owner, "receipt.json")?) != bound.receipt_hash
            {
                report.issue("delivery.metadata-changed", None);
            }
            let linked = open_member(&anchor.file, Path::new(&candidate.id), true)?;
            if directory(&linked.metadata().map_err(ReadModelError::Io)?)
                != directory(&owner.metadata().map_err(ReadModelError::Io)?)
            {
                return Err(invalid());
            }
            stable_root(&anchor.path, &anchor.file)?;
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

#[cfg(all(test, unix))]
mod catalog_tests {
    use super::*;
    #[test]
    fn selected_directory_must_still_match_its_discovered_identity() {
        let root = tempfile::TempDir::new().unwrap();
        let saved = root.path().join("saved");
        std::fs::create_dir(&saved).unwrap();
        std::fs::write(saved.join("receipt.json"), b"{}").unwrap();
        let model = ConsoleReadModel::new(root.path()).unwrap();
        let selected = model.delivery_catalog().unwrap().select("saved").unwrap();
        std::fs::rename(&saved, root.path().join("preserved")).unwrap();
        std::fs::create_dir(&saved).unwrap();
        std::fs::write(saved.join("receipt.json"), b"{}").unwrap();
        assert!(matches!(
            selected.read(),
            Err(ReadModelError::InvalidDocument)
        ));
    }
}
