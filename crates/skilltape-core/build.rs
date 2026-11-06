use std::path::PathBuf;
use std::process::Command;

fn git(root: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let present = root.join(".git").exists();
    let source = present
        .then(|| git(&root, &["rev-parse", "HEAD"]))
        .flatten();
    let dirty = !present
        || git(
            &root,
            &["status", "--porcelain", "--untracked-files=normal"],
        )
        .is_none_or(|v| !v.is_empty());
    let channel = std::env::var("SKILLTAPE_BUILD_CHANNEL").unwrap_or_else(|_| "source".into());
    assert!(
        channel == "source" || channel == "release",
        "invalid build channel"
    );
    if channel == "release" {
        let reference =
            std::env::var("GITHUB_REF").expect("release build requires an immutable workflow tag");
        assert!(
            reference.starts_with("refs/tags/v") && !dirty,
            "release source must be a clean tag"
        );
        assert_eq!(
            git(&root, &["rev-parse", &format!("{reference}^{{commit}}")]),
            source
        );
    }
    println!(
        "cargo:rustc-env=SKILLTAPE_BUILD_SOURCE={}",
        source.unwrap_or_default()
    );
    println!("cargo:rustc-env=SKILLTAPE_BUILD_DIRTY={dirty}");
    println!("cargo:rustc-env=SKILLTAPE_BUILD_CHANNEL={channel}");
    println!(
        "cargo:rustc-env=SKILLTAPE_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    println!("cargo:rerun-if-env-changed=SKILLTAPE_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=GITHUB_REF");
    println!("cargo:rerun-if-changed={}", root.join(".git").display());
    if let Some(files) = git(&root, &["ls-files"]) {
        for file in files.lines() {
            println!("cargo:rerun-if-changed={}", root.join(file).display());
        }
    }
    if let Some(head) = git(&root, &["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={}", root.join(head).display());
    }
    if let Some(reference) = git(&root, &["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&root, &["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={}", root.join(path).display());
        }
    }
    if let Some(index) = git(&root, &["rev-parse", "--git-path", "index"]) {
        println!("cargo:rerun-if-changed={}", root.join(index).display());
    }
}
