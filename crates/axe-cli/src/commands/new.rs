//! `axe new` — scaffold rules, projects, tests.

use std::process::ExitCode;

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// What to create: project, rule, test, util
    pub entity: String,
}

pub fn execute(args: NewArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, "executing new command");
    eprintln!("axe new: not yet implemented");
    Ok(ExitCode::SUCCESS)
}
