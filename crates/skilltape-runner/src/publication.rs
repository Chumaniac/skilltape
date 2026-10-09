//! Directory publication shared by replay materialization and local deliveries.

use std::io;
use std::path::Path;

/// Publish a complete directory without replacing another caller's destination.
pub fn publish_directory_noreplace(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid publication path"))?;
        let destination = CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid publication path"))?;
        // Both paths remain allocated for the syscall; exclusive flags close the
        // existence-check/rename gap without replacing another caller's directory.
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (source, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exclusive directory publication unavailable",
        ))
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_destination_created_after_preflight_is_not_replaced() {
        let temp = tempfile::tempdir().expect("temporary publication");
        let source = temp.path().join("staging");
        let destination = temp.path().join("published");
        fs::create_dir(&source).expect("staging");
        fs::write(source.join("data.txt"), "completed material").expect("material");
        assert!(!destination.exists());
        fs::create_dir(&destination).expect("another caller wins after preflight");
        let error = publish_directory_noreplace(&source, &destination).expect_err("no overwrite");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(source.join("data.txt").is_file());
        assert_eq!(
            fs::read_dir(&destination)
                .expect("existing directory")
                .count(),
            0
        );
    }
}
