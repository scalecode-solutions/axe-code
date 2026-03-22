//! `axe completions` — shell completion generation.

use std::io;
use std::process::ExitCode;

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::Cli;

#[derive(clap::Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
    pub shell: String,
}

pub fn execute(args: CompletionsArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, "executing completions command");

    let shell: Shell = match args.shell.to_lowercase().as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "elvish" => Shell::Elvish,
        "powershell" | "ps" | "pwsh" => Shell::PowerShell,
        other => {
            eprintln!("axe completions: unsupported shell '{other}'");
            eprintln!("supported: bash, zsh, fish, elvish, powershell");
            return Ok(ExitCode::from(1));
        }
    };

    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "axe", &mut io::stdout());

    Ok(ExitCode::SUCCESS)
}
