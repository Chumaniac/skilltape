use std::path::PathBuf;
use std::process::ExitCode;

use skilltape_core::SkillPackage;
use skilltape_export::{exporter_for, ExportManifest};

const INPUT_ERROR_EXIT_CODE: u8 = 2;
const POLICY_ERROR_EXIT_CODE: u8 = 3;

#[derive(Debug)]
pub(crate) struct ExportConfig {
    pub skill_path: PathBuf,
    pub target: String,
    pub output: PathBuf,
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

    let manifest = match exporter.export(&package, &config.output) {
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

fn print_human_summary(manifest: &ExportManifest, output: &std::path::Path) {
    println!("Exported {} to {}", manifest.target, output.display());
    println!("Package hash: {}", manifest.package_hash);
    println!("Files: {}", manifest.files.len());
}
