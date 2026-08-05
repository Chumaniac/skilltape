use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use skilltape_core::create_skill_template;

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
        Command::Lint { path, strict, json } => {
            let _ = (path, strict, json);
            ExitCode::SUCCESS
        }
    }
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
