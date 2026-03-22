//! `axe lsp` — language server mode.

use std::process::ExitCode;

#[derive(clap::Args, Debug)]
pub struct LspArgs {
    /// Use stdio transport
    #[arg(long, default_value_t = true)]
    pub stdio: bool,
}

pub fn execute(args: LspArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, "executing lsp command");
    eprintln!("axe lsp: not yet implemented");
    Ok(ExitCode::SUCCESS)
}
