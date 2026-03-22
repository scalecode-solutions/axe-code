//! `axe test` — rule testing and snapshot validation.

use std::process::ExitCode;

#[derive(clap::Args, Debug)]
pub struct TestArgs {
    /// Update all snapshots
    #[arg(long)]
    pub update_all: bool,

    /// Rule file or directory to test
    #[arg(short, long)]
    pub rule: Option<String>,
}

pub fn execute(args: TestArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, "executing test command");
    eprintln!("axe test: not yet implemented");
    Ok(ExitCode::SUCCESS)
}
