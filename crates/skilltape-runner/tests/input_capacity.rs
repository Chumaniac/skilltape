use std::fs;
use std::path::Path;

use skilltape_runner::{preflight_input_capacity, RunError};
use tempfile::tempdir;

const MIB: u64 = 1024 * 1024;

fn sparse(path: &Path, size: u64) {
    fs::File::create(path)
        .expect("synthetic file")
        .set_len(size)
        .expect("synthetic length");
}

fn assert_capacity(root: &Path, expected_dimension: &str, expected_limit: u64) {
    let error = preflight_input_capacity(root).expect_err("capacity exceeded");
    assert!(matches!(
        error,
        RunError::InputCapacity { dimension, limit }
        if dimension == expected_dimension && limit == expected_limit
    ));
}

#[test]
fn accepts_empty_and_small_nested_inputs() {
    let root = tempdir().expect("root");
    preflight_input_capacity(root.path()).expect("empty input");
    fs::create_dir(root.path().join("notes")).expect("notes");
    fs::write(root.path().join("notes/source.md"), "synthetic source").expect("source");
    preflight_input_capacity(root.path()).expect("small nested input");
}

#[test]
fn accepts_exact_file_and_total_byte_limits_without_reading_contents() {
    let root = tempdir().expect("root");
    for index in 0..4 {
        sparse(&root.path().join(format!("part-{index}.csv")), 16 * MIB);
    }
    preflight_input_capacity(root.path()).expect("inclusive byte limits");
}

#[test]
fn rejects_one_byte_over_file_limit() {
    let root = tempdir().expect("root");
    sparse(&root.path().join("change.diff"), 16 * MIB + 1);
    assert_capacity(root.path(), "file_bytes", 16 * MIB);
}

#[test]
fn rejects_aggregate_limit_with_each_file_individually_valid() {
    let root = tempdir().expect("root");
    for index in 0..4 {
        sparse(&root.path().join(format!("part-{index}.csv")), 16 * MIB);
    }
    sparse(&root.path().join("extra.csv"), 1);
    assert_capacity(root.path(), "total_bytes", 64 * MIB);
}

#[test]
fn counts_empty_directories_in_the_inclusive_entry_budget() {
    let root = tempdir().expect("root");
    for index in 0..10_000 {
        fs::create_dir(root.path().join(format!("folder-{index}"))).expect("empty directory");
    }
    preflight_input_capacity(root.path()).expect("exact entry limit");
    fs::create_dir(root.path().join("extra-folder")).expect("extra directory");
    assert_capacity(root.path(), "entries", 10_000);
}

#[test]
fn rejects_entry_beyond_depth_64_including_a_file_leaf() {
    let root = tempdir().expect("root");
    let mut current = root.path().to_path_buf();
    for _ in 0..64 {
        current.push("d");
        fs::create_dir(&current).expect("nested directory");
    }
    preflight_input_capacity(root.path()).expect("depth 64");
    fs::write(current.join("source.md"), "x").expect("depth 65 file");
    assert_capacity(root.path(), "depth", 64);
}

#[test]
fn rejects_a_file_as_input_root() {
    let root = tempdir().expect("root");
    let file = root.path().join("source.md");
    sparse(&file, 0);
    assert!(matches!(
        preflight_input_capacity(&file),
        Err(RunError::InvalidInputRoot { .. })
    ));
}

#[test]
fn rejects_parent_components_before_input_metadata_access() {
    let root = tempdir().expect("root");
    fs::create_dir(root.path().join("nested")).expect("nested");
    fs::create_dir(root.path().join("selected")).expect("selected");
    let indirect = root.path().join("nested/../selected");
    assert!(preflight_input_capacity(&indirect).is_err());
}

#[test]
fn accepts_a_direct_relative_input_root() {
    let current = std::env::current_dir().expect("current directory");
    let root = tempfile::tempdir_in(&current).expect("relative fixture");
    fs::write(root.path().join("note.md"), "synthetic").expect("note");
    let relative = root
        .path()
        .strip_prefix(&current)
        .expect("relative selection");
    preflight_input_capacity(relative).expect("direct relative input");
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_entries_and_ancestors_without_following_them() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("root");
    let source = root.path().join("source");
    fs::create_dir(&source).expect("source");
    sparse(&source.join("note.md"), 0);
    let link = root.path().join("source-link");
    symlink(&source, &link).expect("ancestor symlink");
    assert!(preflight_input_capacity(&link).is_err());
    let nested = source.join("nested");
    fs::create_dir(&nested).expect("nested");
    assert!(preflight_input_capacity(&link.join("nested")).is_err());
    symlink(source.join("note.md"), source.join("note-link.md")).expect("file symlink");
    assert!(preflight_input_capacity(&source).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_a_fifo_without_opening_it() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let root = tempdir().expect("root");
    let path = root.path().join("synthetic-pipe");
    let encoded = CString::new(path.as_os_str().as_bytes()).expect("FIFO path");
    // Create an inert local special entry; preflight must never open/read it.
    assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
    let error = preflight_input_capacity(root.path()).expect_err("unsupported entry");
    assert!(error.to_string().contains("unsupported input entry"));
}
