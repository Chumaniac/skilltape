use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use skilltape_core::{create_skill_template, Diagnostic, DiagnosticLevel, LintReport, SkillPackage};

#[derive(Parser)]
#[command(name = "skilltape")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init {
        name: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        force: bool,
    },
    Lint {
        path: PathBuf,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
    },
}

pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Init {
            name,
            output,
            force,
        } => init(name, output, force),
        Command::Lint { path, strict, json } => lint(path, strict, json),
    }
}

fn lint(path: PathBuf, strict: bool, json: bool) -> ExitCode {
    let package = match SkillPackage::load(&path) {
        Ok(package) => package,
        Err(error) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"error": error.to_string()})
                );
            } else {
                eprintln!("{error}");
            }
            return ExitCode::FAILURE;
        }
    };

    let report = package.lint(strict);
    if json {
        println!("{}", report_json(&report));
    } else {
        print_text_report(&report);
    }

    if report.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn print_text_report(report: &LintReport) {
    for diagnostic in report.errors.iter().chain(report.warnings.iter()) {
        let level = match diagnostic.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
        };
        println!(
            "{level}[{}] {}:{}\n  {}",
            diagnostic.code, diagnostic.file, diagnostic.path, diagnostic.message
        );
    }

    if report.errors.is_empty() && report.warnings.is_empty() {
        println!("Lint passed: {} files checked", report.files_checked);
    } else {
        println!(
            "Checked {} files: {} errors, {} warnings",
            report.files_checked,
            report.errors.len(),
            report.warnings.len()
        );
    }
}

fn report_json(report: &LintReport) -> String {
    serde_json::to_string(&serde_json::json!({
        "files_checked": report.files_checked,
        "errors": report.errors.iter().map(diagnostic_json).collect::<Vec<_>>(),
        "warnings": report.warnings.iter().map(diagnostic_json).collect::<Vec<_>>(),
    }))
    .expect("lint report JSON serialization cannot fail")
}

fn diagnostic_json(diagnostic: &Diagnostic) -> serde_json::Value {
    serde_json::json!({
        "code": diagnostic.code,
        "level": match diagnostic.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
        },
        "file": diagnostic.file,
        "path": diagnostic.path,
        "message": diagnostic.message,
    })
}

fn init(name: String, output: PathBuf, force: bool) -> ExitCode {
    if output.exists() && !force {
        eprintln!("target already exists: {}", output.display());
        return ExitCode::FAILURE;
    }

    match create_skill_template(&output, &name) {
        Ok(()) => {
            println!("Initialized {} at {}", name, output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
