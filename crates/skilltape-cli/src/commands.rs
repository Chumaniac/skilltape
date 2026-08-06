use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use skilltape_core::{create_skill_template, Diagnostic, LintReport, SkillPackage};

use crate::output;

#[path = "capture_command.rs"]
mod capture_command;
#[path = "compile_command.rs"]
mod compile_command;

const PACKAGE_ERROR_EXIT_CODE: u8 = 2;
const POLICY_ERROR_EXIT_CODE: u8 = 3;

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
    Capture {
        name: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, action = clap::ArgAction::Append)]
        allow_env: Vec<String>,
        #[arg(long, default_value_t = 64 * 1024)]
        max_output_bytes: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        yes: bool,
    },
    Compile {
        tape: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        accept_proposal: bool,
    },
}

pub fn run() -> ExitCode {
    let _interrupt_guard = if std::env::args().nth(1).as_deref() == Some("capture") {
        Some(capture_command::InterruptGuard::install())
    } else {
        None
    };
    match Cli::parse().command {
        Command::Init {
            name,
            output,
            force,
        } => init(name, output, force),
        Command::Lint { path, strict, json } => lint(path, strict, json),
        Command::Capture {
            name,
            workspace,
            command,
            output,
            allow_env,
            max_output_bytes,
            json,
            yes,
        } => capture_command::run(capture_command::CaptureConfig {
            name,
            workspace,
            command,
            output,
            allow_env,
            max_output_bytes,
            json,
            yes,
        }),
        Command::Compile {
            tape,
            output,
            provider,
            accept_proposal,
        } => compile_command::run(compile_command::CompileConfig {
            tape,
            output,
            provider,
            accept_proposal,
        }),
    }
}

fn lint(path: PathBuf, strict: bool, json: bool) -> ExitCode {
    let package = match SkillPackage::load(&path) {
        Ok(package) => package,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    let report = package.lint(strict);
    if json {
        println!("{}", output::json_report(&report));
    } else {
        print!("{}", output::human_report(&report));
    }

    lint_exit_code(&report)
}

fn lint_exit_code(report: &LintReport) -> ExitCode {
    if report.errors.is_empty() {
        return ExitCode::SUCCESS;
    }

    if report.errors.iter().any(is_package_or_schema_failure) {
        ExitCode::from(PACKAGE_ERROR_EXIT_CODE)
    } else {
        ExitCode::from(POLICY_ERROR_EXIT_CODE)
    }
}

fn is_package_or_schema_failure(diagnostic: &Diagnostic) -> bool {
    matches!(diagnostic.code.as_str(), "PKG001" | "PKG002" | "PKG003")
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
