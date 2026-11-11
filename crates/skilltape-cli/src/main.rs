mod commands;
mod interrupt;
mod output;

fn main() -> std::process::ExitCode {
    commands::run()
}
