//! `axe completions` — shell completion generation.

use std::process::ExitCode;

#[derive(clap::Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: String,
}

pub fn execute(args: CompletionsArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, "executing completions command");
    eprintln!("axe completions: not yet implemented");
    Ok(ExitCode::SUCCESS)
}
