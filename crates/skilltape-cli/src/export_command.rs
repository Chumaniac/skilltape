use std::path::PathBuf;
use std::process::ExitCode;

use skilltape_core::SkillPackage;
use skilltape_export::{export_with_receipt, exporter_for, ExportManifest};
use std::fs;
use std::io::Read;

const INPUT_ERROR_EXIT_CODE: u8 = 2;
const POLICY_ERROR_EXIT_CODE: u8 = 3;

#[derive(Debug)]
pub(crate) struct ExportConfig {
    pub skill_path: PathBuf,
    pub target: String,
    pub output: PathBuf,
    pub receipt: Option<PathBuf>,
    pub json: bool,
}

pub(crate) fn run(config: ExportConfig) -> ExitCode {
    let package = match SkillPackage::load(&config.skill_path) {
        Ok(package) => package,
        Err(error) => {
            eprintln!("skill package failed to load: {error}");
            return ExitCode::from(INPUT_ERROR_EXIT_CODE);
        }
    };

    let exporter = match exporter_for(&config.target) {
        Ok(exporter) => exporter,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(POLICY_ERROR_EXIT_CODE);
        }
    };

    let result = if let Some(path) = &config.receipt {
        match read_receipt(path) {
            Ok(bytes) => export_with_receipt(exporter.as_ref(), &package, &config.output, &bytes),
            Err(_) => {
                eprintln!("Receipt input is not a stable bounded regular file");
                return ExitCode::from(INPUT_ERROR_EXIT_CODE);
            }
        }
    } else {
        exporter.export(&package, &config.output)
    };
    let manifest = match result {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(POLICY_ERROR_EXIT_CODE);
        }
    };

    if config.json {
        println!(
            "{}",
            serde_json::to_string(&manifest).expect("export manifest JSON serialization")
        );
    } else {
        print_human_summary(&manifest, &config.output);
    }
    ExitCode::SUCCESS
}

fn read_receipt(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    const LIMIT: u64 = 1024 * 1024;
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() > LIMIT {
        return Err(std::io::Error::other("invalid Receipt file"));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    let file = options.open(path)?;
    let observed = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if observed.dev() != before.dev() || observed.ino() != before.ino() {
            return Err(std::io::Error::other("Receipt changed"));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if observed.file_attributes() & 0x400 != 0 {
            return Err(std::io::Error::other("invalid Receipt file"));
        }
    }
    if !observed.is_file()
        || observed.len() != before.len()
        || observed.modified()? != before.modified()?
    {
        return Err(std::io::Error::other("Receipt changed"));
    }
    let mut bytes = Vec::new();
    file.take(before.len() + 1).read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if after.dev() != before.dev()
            || after.ino() != before.ino()
            || after.ctime() != before.ctime()
            || after.ctime_nsec() != before.ctime_nsec()
        {
            return Err(std::io::Error::other("Receipt changed"));
        }
    }
    if bytes.len() as u64 != before.len()
        || after.file_type().is_symlink()
        || after.len() != before.len()
        || after.modified()? != before.modified()?
    {
        return Err(std::io::Error::other("Receipt changed"));
    }
    Ok(bytes)
}

fn print_human_summary(manifest: &ExportManifest, output: &std::path::Path) {
    println!("Exported {} to {}", manifest.target, output.display());
    println!("Package hash: {}", manifest.package_hash);
    println!("Files: {}", manifest.files.len());
}
