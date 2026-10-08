//! Input-only bounded hashing and staging. Generic package/output copies keep their own policy.

use crate::workspace::{
    ensure_no_symlink_ancestors, input_snapshot, InputSnapshot, WorkspaceError,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

fn failure(path: &Path, source: io::Error) -> WorkspaceError {
    WorkspaceError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "input changed during bounded I/O",
    )
}

fn same_metadata(left: &Metadata, right: &Metadata) -> io::Result<bool> {
    if left.is_file() != right.is_file()
        || left.is_dir() != right.is_dir()
        || left.len() != right.len()
        || left.modified()? != right.modified()?
    {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev()
            || left.ino() != right.ino()
            || left.ctime() != right.ctime()
            || left.ctime_nsec() != right.ctime_nsec()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn open_input(path: &Path, expected: &Metadata) -> Result<File, WorkspaceError> {
    ensure_no_symlink_ancestors(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Nonblocking prevents a regular-file-to-FIFO race from blocking setup.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path).map_err(|error| failure(path, error))?;
    let metadata = file.metadata().map_err(|error| failure(path, error))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(failure(path, changed()));
        }
    }
    if !metadata.is_file()
        || !same_metadata(expected, &metadata).map_err(|error| failure(path, error))?
    {
        return Err(failure(path, changed()));
    }
    Ok(file)
}

/// Read exactly the observed size plus at most one probe byte. Never read a growing file to EOF.
fn read_observed(
    reader: &mut impl Read,
    size: u64,
    mut consume: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let mut remaining = size;
    let mut buffer = [0u8; 8192];
    while remaining > 0 {
        let limit = remaining.min(buffer.len() as u64) as usize;
        let count = match reader.read(&mut buffer[..limit]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Err(changed());
        }
        consume(&buffer[..count])?;
        remaining -= count as u64;
    }
    loop {
        match reader.read(&mut buffer[..1]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Ok(0) => return Ok(()),
            Ok(_) => return Err(changed()),
            Err(error) => return Err(error),
        }
    }
}

fn check_snapshot(snapshot: &InputSnapshot) -> Result<(), WorkspaceError> {
    let current = input_snapshot(&snapshot.root)?;
    if !same_metadata(&snapshot.metadata, &current.metadata)
        .map_err(|error| failure(&snapshot.root, error))?
        || snapshot.entries.len() != current.entries.len()
    {
        return Err(failure(&snapshot.root, changed()));
    }
    for ((path, metadata), (new_path, new_metadata)) in
        snapshot.entries.iter().zip(&current.entries)
    {
        if path != new_path
            || !same_metadata(metadata, new_metadata).map_err(|error| failure(path, error))?
        {
            return Err(failure(path, changed()));
        }
    }
    Ok(())
}

pub(crate) fn digest_input_tree(root: &Path) -> Result<String, WorkspaceError> {
    let snapshot = input_snapshot(root)?;
    // Preserve the legacy globally sorted UTF-8 relative path order.
    let mut files: Vec<_> = snapshot
        .entries
        .iter()
        .filter(|(_, metadata)| metadata.is_file())
        .map(|(path, metadata)| {
            (
                path.strip_prefix(&snapshot.root)
                    .expect("rooted inventory")
                    .to_string_lossy()
                    .replace('\\', "/"),
                path,
                metadata,
            )
        })
        .collect();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut digest = Sha256::new();
    for (relative, path, metadata) in files {
        let mut file = open_input(path, metadata)?;
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(metadata.len().to_be_bytes());
        read_observed(&mut file, metadata.len(), |bytes| {
            digest.update(bytes);
            Ok(())
        })
        .map_err(|error| failure(path, error))?;
    }
    check_snapshot(&snapshot)?;
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn copy_input_tree(root: &Path, destination: &Path) -> Result<(), WorkspaceError> {
    let snapshot = input_snapshot(root)?;
    ensure_no_symlink_ancestors(destination)?;
    fs::create_dir(destination).map_err(|error| failure(destination, error))?;
    for (path, metadata) in &snapshot.entries {
        let target = destination.join(path.strip_prefix(&snapshot.root).expect("rooted inventory"));
        ensure_no_symlink_ancestors(&target)?;
        if metadata.is_dir() {
            fs::create_dir(&target).map_err(|error| failure(&target, error))?;
        } else {
            let mut source = open_input(path, metadata)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|error| failure(&target, error))?;
            read_observed(&mut source, metadata.len(), |bytes| output.write_all(bytes))
                .map_err(|error| failure(path, error))?;
            fs::set_permissions(&target, metadata.permissions())
                .map_err(|error| failure(&target, error))?;
        }
    }
    check_snapshot(&snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn infinite_growth_reads_only_observed_size_plus_one_and_never_writes_probe() {
        struct Growing {
            read: usize,
        }
        impl Read for Growing {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                assert!(
                    self.read + buffer.len() <= 4,
                    "growing input was read beyond the probe"
                );
                buffer.fill(b'x');
                self.read += buffer.len();
                Ok(buffer.len())
            }
        }
        let mut source = Growing { read: 0 };
        let mut written = Vec::new();
        let error = read_observed(&mut source, 3, |bytes| {
            written.extend_from_slice(bytes);
            Ok(())
        })
        .expect_err("growth");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(written, b"xxx");
        assert_eq!(source.read, 4);
        let mut empty_written = Vec::new();
        assert!(read_observed(&mut Growing { read: 0 }, 0, |bytes| {
            empty_written.extend_from_slice(bytes);
            Ok(())
        })
        .is_err());
        assert!(empty_written.is_empty());
    }

    #[test]
    fn a_real_file_growing_during_read_does_not_expand_the_copy() {
        let root = tempdir().expect("root");
        let path = root.path().join("data");
        fs::write(&path, vec![b'x'; 8192]).expect("data");
        let snapshot = input_snapshot(root.path()).expect("inventory");
        let mut input = open_input(&path, &snapshot.entries[0].1).expect("input");
        let mut copied = Vec::new();
        let result = read_observed(&mut input, 8192, |bytes| {
            copied.extend_from_slice(bytes);
            OpenOptions::new()
                .append(true)
                .open(&path)?
                .write_all(b"additional data")
        });
        assert_eq!(
            result.expect_err("concurrent growth").kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(copied.len(), 8192);
        assert!(check_snapshot(&snapshot).is_err());
    }

    #[test]
    fn stable_and_truncated_reads_have_distinct_outcomes() {
        assert!(read_observed(&mut Cursor::new(b"abc"), 3, |_| Ok(())).is_ok());
        assert!(read_observed(&mut Cursor::new(b"ab"), 3, |_| Ok(())).is_err());
        assert!(read_observed(&mut Cursor::new(b""), 0, |_| Ok(())).is_ok());
    }

    #[test]
    fn bounded_hash_keeps_the_existing_digest_and_copy_contents() {
        let root = tempdir().expect("root");
        fs::create_dir(root.path().join("a")).expect("nested");
        fs::write(root.path().join("a.txt"), b"first\n").expect("first");
        fs::write(root.path().join("a/second.txt"), b"second\n").expect("second");
        fs::write(root.path().join("large.bin"), vec![b'A'; 256 * 8192 + 17]).expect("binary");
        assert_eq!(
            digest_input_tree(root.path()).expect("digest"),
            "81956d7feb1aadf66e9e6405958c27581381b8851c27d4217cab9ba817584f6b"
        );
        let destination = tempdir().expect("destination");
        let copy = destination.path().join("inputs");
        copy_input_tree(root.path(), &copy).expect("bounded copy");
        assert_eq!(
            digest_input_tree(&copy).expect("copy digest"),
            digest_input_tree(root.path()).expect("input digest")
        );
    }

    #[test]
    fn additions_and_replacements_after_inventory_are_rejected() {
        let root = tempdir().expect("root");
        fs::write(root.path().join("data"), b"old").expect("data");
        let snapshot = input_snapshot(root.path()).expect("inventory");
        fs::write(root.path().join("new"), b"x").expect("added input");
        assert!(check_snapshot(&snapshot).is_err());
        fs::remove_file(root.path().join("new")).expect("remove added");
        fs::write(root.path().join("replacement"), b"new").expect("replacement");
        fs::rename(root.path().join("replacement"), root.path().join("data")).expect("replace");
        assert!(open_input(&snapshot.entries[0].0, &snapshot.entries[0].1).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_leaf_replaced_by_a_symlink_or_fifo_is_never_read() {
        use std::ffi::CString;
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        let root = tempdir().expect("root");
        let path = root.path().join("data");
        fs::write(&path, b"old").expect("data");
        let snapshot = input_snapshot(root.path()).expect("inventory");
        let expected = &snapshot.entries[0].1;
        fs::remove_file(&path).expect("remove");
        let outside = tempdir().expect("outside");
        fs::write(outside.path().join("data"), b"old").expect("outside fixture");
        symlink(outside.path().join("data"), &path).expect("symlink");
        assert!(open_input(&path, expected).is_err());
        fs::remove_file(&path).expect("remove symlink");
        let encoded = CString::new(path.as_os_str().as_bytes()).expect("FIFO path");
        assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
        assert!(open_input(&path, expected).is_err());
    }
}
