//! `axe scan` — config-driven multi-rule scanning.

use crate::output::OutputFormat;
use std::process::ExitCode;

/// Arguments for `axe scan`.
#[derive(clap::Args, Debug)]
pub struct ScanArgs {
    /// Rule file or directory
    #[arg(short, long)]
    pub rule: Option<String>,

    /// Config file path (default: axeconfig.yml)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Files or directories to scan
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Filter by severity
    #[arg(long)]
    pub severity: Option<String>,

    /// Maximum number of results
    #[arg(long)]
    pub max_results: Option<usize>,
}

pub fn execute(args: ScanArgs, format: OutputFormat) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, ?format, "executing scan command");

    // TODO: implement scan command
    // 1. Load config from axeconfig.yml or --config
    // 2. Load rules from --rule or config's rule directories
    // 3. Build CombinedScan with kind pre-filtering
    // 4. Walk files, parse, scan with combined scanner
    // 5. Handle suppressions (// axe-ignore)
    // 6. Output in requested format

    eprintln!("axe scan: not yet implemented");
    Ok(ExitCode::SUCCESS)
}
